use crate::policy::encode_percent;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use zeroize::Zeroizing;

const REDACTED: &[u8] = b"[redacted]";

pub struct Scrubber {
    patterns: Vec<Zeroizing<Vec<u8>>>,
    longest: usize,
    held: Zeroizing<Vec<u8>>,
}

impl Scrubber {
    pub fn new(secret: &str) -> Scrubber {
        let mut patterns: Vec<Zeroizing<Vec<u8>>> = [
            secret.to_string(),
            STANDARD.encode(secret),
            STANDARD_NO_PAD.encode(secret),
            URL_SAFE.encode(secret),
            URL_SAFE_NO_PAD.encode(secret),
            encode_percent(secret),
            encode_percent(secret).replace("%20", "+"),
        ]
        .into_iter()
        .filter(|form| !form.is_empty())
        .map(|form| Zeroizing::new(form.into_bytes()))
        .collect();
        patterns.sort_by_key(|form| std::cmp::Reverse(form.len()));
        patterns.dedup_by(|a, b| a.as_slice() == b.as_slice());
        let longest = patterns.first().map_or(0, |form| form.len());
        Scrubber {
            patterns,
            longest,
            held: Zeroizing::new(Vec::new()),
        }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<u8> {
        self.held.extend_from_slice(chunk);
        self.scan(false)
    }

    pub fn finish(&mut self) -> Vec<u8> {
        self.scan(true)
    }

    pub fn text(&self, text: &str) -> String {
        let mut fresh = Scrubber {
            patterns: self.patterns.clone(),
            longest: self.longest,
            held: Zeroizing::new(text.as_bytes().to_vec()),
        };
        String::from_utf8_lossy(&fresh.scan(true)).into_owned()
    }

    fn scan(&mut self, last: bool) -> Vec<u8> {
        let buf = Zeroizing::new(std::mem::take(&mut *self.held));
        let mut out = Vec::with_capacity(buf.len());
        let mut at = 0;
        while at < buf.len() {
            let rest = &buf[at..];
            if let Some(form) = self.patterns.iter().find(|form| rest.starts_with(form)) {
                out.extend_from_slice(REDACTED);
                at += form.len();
                continue;
            }
            if !last && rest.len() < self.longest && self.partial(rest) {
                break;
            }
            out.push(buf[at]);
            at += 1;
        }
        *self.held = buf[at..].to_vec();
        out
    }

    fn partial(&self, rest: &[u8]) -> bool {
        self.patterns
            .iter()
            .any(|form| form.len() > rest.len() && form.starts_with(rest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn through(secret: &str, chunks: &[&[u8]]) -> (String, Vec<Vec<u8>>) {
        let mut scrubber = Scrubber::new(secret);
        let mut parts: Vec<Vec<u8>> = chunks.iter().map(|chunk| scrubber.push(chunk)).collect();
        parts.push(scrubber.finish());
        (String::from_utf8(parts.concat()).unwrap(), parts)
    }

    #[test]
    fn a_secret_split_across_chunks_is_still_caught() {
        let (all, _) = through("sk_live_42", &[b"a sk_li", b"ve_42 b sk_", b"live_42"]);
        assert_eq!(all, "a [redacted] b [redacted]");
        let encoded = STANDARD.encode("sk_live_42");
        let (all, _) = through(
            "sk_live_42",
            &[&encoded.as_bytes()[..5], &encoded.as_bytes()[5..]],
        );
        assert_eq!(all, "[redacted]");
        let (all, _) = through("a b/c", &[b"q=a%20b%2", b"Fc&r=a+b%2Fc"]);
        assert_eq!(all, "q=[redacted]&r=[redacted]");
    }

    #[test]
    fn text_that_cannot_start_a_secret_is_not_held_back() {
        let (_, parts) = through("sk_live_42", &[b"data: one\n\n", b"data: two s"]);
        assert_eq!(parts[0], b"data: one\n\n");
        assert_eq!(parts[1], b"data: two ");
        assert_eq!(parts[2], b"s");
        assert_eq!(
            Scrubber::new("tok").text("Bearer tok, dG9r"),
            "Bearer [redacted], [redacted]"
        );
    }
}
