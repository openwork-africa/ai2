use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const BASE: Duration = Duration::from_secs(30);
pub const LIMIT: Duration = Duration::from_secs(600);

#[derive(Debug, PartialEq)]
pub enum Refusal {
    Busy,
    Throttled(Duration),
}

struct State {
    busy: bool,
    denials: u32,
    until: Option<Instant>,
}

pub struct Guard {
    base: Duration,
    limit: Duration,
    state: Mutex<State>,
}

pub struct Turn<'a> {
    guard: &'a Guard,
    done: bool,
}

impl Guard {
    pub fn new(base: Duration, limit: Duration) -> Guard {
        Guard {
            base,
            limit,
            state: Mutex::new(State {
                busy: false,
                denials: 0,
                until: None,
            }),
        }
    }

    pub fn enter(&self, now: Instant) -> Result<Turn<'_>, Refusal> {
        let mut state = self.state.lock().expect("guard lock");
        if let Some(left) = state
            .until
            .map(|until| until.saturating_duration_since(now))
            .filter(|left| !left.is_zero())
        {
            return Err(Refusal::Throttled(left));
        }
        if state.busy {
            return Err(Refusal::Busy);
        }
        state.busy = true;
        Ok(Turn {
            guard: self,
            done: false,
        })
    }

    fn wait_for(&self, denials: u32) -> Duration {
        let doubled = self
            .base
            .checked_mul(
                1u32.checked_shl(denials.saturating_sub(1))
                    .unwrap_or(u32::MAX),
            )
            .unwrap_or(self.limit);
        doubled.min(self.limit)
    }

    fn finish(&self, approved: bool, now: Instant) {
        let mut state = self.state.lock().expect("guard lock");
        state.busy = false;
        if approved {
            state.denials = 0;
            state.until = None;
        } else {
            state.denials = state.denials.saturating_add(1);
            state.until = Some(now + self.wait_for(state.denials));
        }
    }
}

impl Turn<'_> {
    pub fn finish(mut self, approved: bool, now: Instant) {
        self.done = true;
        self.guard.finish(approved, now);
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        if !self.done {
            self.guard.finish(false, Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn one_prompt_at_a_time() {
        let guard = Guard::new(BASE, LIMIT);
        let now = Instant::now();
        let turn = guard.enter(now).unwrap();
        assert_eq!(guard.enter(now).err(), Some(Refusal::Busy));
        turn.finish(true, now);
        guard.enter(now).unwrap().finish(true, now);
    }

    #[test]
    fn denials_in_a_row_double_the_wait_up_to_ten_minutes() {
        let guard = Guard::new(BASE, LIMIT);
        let mut now = Instant::now();
        let mut waits = Vec::new();
        for _ in 0..7 {
            guard.enter(now).unwrap().finish(false, now);
            let Err(Refusal::Throttled(left)) = guard.enter(now) else {
                panic!("not throttled");
            };
            assert_eq!(
                guard.enter(now + left - secs(1)).err(),
                Some(Refusal::Throttled(secs(1)))
            );
            waits.push(left.as_secs());
            now += left;
        }
        assert_eq!(waits, [30, 60, 120, 240, 480, 600, 600]);
        guard.enter(now).unwrap().finish(true, now);
        guard.enter(now).unwrap().finish(false, now);
        assert_eq!(guard.enter(now).err(), Some(Refusal::Throttled(secs(30))));
    }

    #[test]
    fn a_turn_dropped_without_an_answer_counts_as_a_denial() {
        let guard = Guard::new(BASE, LIMIT);
        drop(guard.enter(Instant::now()).unwrap());
        assert!(matches!(
            guard.enter(Instant::now()),
            Err(Refusal::Throttled(_))
        ));
        assert_eq!(guard.wait_for(40), LIMIT);
    }
}
