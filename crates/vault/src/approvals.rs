use serde::Serialize;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};
use utoipa::ToSchema;

#[derive(Serialize, Clone, ToSchema)]
pub struct Pending {
    pub id: String,
    pub token: String,
    pub sandbox: String,
    pub key: String,
    pub method: String,
    pub url: String,
    pub intent: String,
    pub request: String,
    pub line: String,
    pub created: String,
}

#[derive(Default)]
pub struct Approvals {
    queue: Mutex<Vec<(Pending, Option<bool>)>>,
    changed: Condvar,
}

impl Approvals {
    pub fn ask(&self, pending: Pending, timeout: Duration) -> Option<bool> {
        let id = pending.id.clone();
        let deadline = Instant::now() + timeout;
        let mut queue = self.queue.lock().expect("approvals lock");
        queue.push((pending, None));
        let decided = loop {
            let decided = queue
                .iter()
                .find(|(p, _)| p.id == id)
                .and_then(|(_, decided)| *decided);
            let left = deadline.saturating_duration_since(Instant::now());
            if decided.is_some() || left.is_zero() {
                break decided;
            }
            queue = self
                .changed
                .wait_timeout(queue, left)
                .expect("approvals lock")
                .0;
        };
        queue.retain(|(p, _)| p.id != id);
        decided
    }

    pub fn list(&self) -> Vec<Pending> {
        self.queue
            .lock()
            .expect("approvals lock")
            .iter()
            .filter(|(_, decided)| decided.is_none())
            .map(|(p, _)| p.clone())
            .collect()
    }

    pub fn line(&self, id: &str) -> Option<String> {
        self.queue
            .lock()
            .expect("approvals lock")
            .iter()
            .find(|(p, decided)| p.id == id && decided.is_none())
            .map(|(p, _)| p.line.clone())
    }

    pub fn decide(&self, id: &str, approve: bool) -> bool {
        let mut queue = self.queue.lock().expect("approvals lock");
        let Some(entry) = queue
            .iter_mut()
            .find(|(p, decided)| p.id == id && decided.is_none())
        else {
            return false;
        };
        entry.1 = Some(approve);
        self.changed.notify_all();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn pending(id: &str) -> Pending {
        Pending {
            id: id.into(),
            token: "tok_1".into(),
            sandbox: "s".into(),
            key: "k".into(),
            method: "POST".into(),
            url: "https://h.io/".into(),
            intent: "i".into(),
            request: "r".into(),
            line: "l".into(),
            created: String::new(),
        }
    }

    #[test]
    fn a_call_waits_for_a_decision_or_times_out() {
        let approvals = Arc::new(Approvals::default());
        assert_eq!(approvals.ask(pending("a"), Duration::from_millis(20)), None);
        assert!(approvals.list().is_empty());
        let waiting = Arc::clone(&approvals);
        let asked = std::thread::spawn(move || waiting.ask(pending("b"), Duration::from_secs(10)));
        while approvals.list().is_empty() {
            std::thread::yield_now();
        }
        assert!(!approvals.decide("nope", true));
        assert!(approvals.decide("b", true));
        assert_eq!(asked.join().unwrap(), Some(true));
    }
}
