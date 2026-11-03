use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{Read, Write};

const MAX_HEAD: usize = 64 * 1024;
const MAX_BODY: usize = 64 * 1024 * 1024;

pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }

    pub fn json(&self) -> Result<Value> {
        if self.body.is_empty() {
            return Ok(Value::Object(Default::default()));
        }
        serde_json::from_slice(&self.body).context("the body is not JSON")
    }

    pub fn single(&self, name: &str) -> Option<&str> {
        let mut found = self
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name));
        match (found.next(), found.next()) {
            (Some((_, value)), None) => Some(value),
            _ => None,
        }
    }

    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }

    pub fn query(&self) -> Option<&str> {
        self.target.split_once('?').map(|(_, q)| q)
    }
}

impl Response {
    pub fn new(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".into(), content_type.into())],
            body: body.into(),
        }
    }

    pub fn json(status: u16, value: &Value) -> Self {
        Self::new(status, "application/json", value.to_string())
    }

    pub fn error(status: u16, code: &str, message: &str) -> Self {
        Self::json(status, &error(code, message))
    }

    pub fn with(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }

    pub fn write_to(&self, out: &mut impl Write) -> std::io::Result<()> {
        let mut headers = self.headers.clone();
        headers.push(("Content-Length".into(), self.body.len().to_string()));
        headers.push(("Connection".into(), "close".into()));
        write_head(out, self.status, &headers)?;
        out.write_all(&self.body)?;
        out.flush()
    }
}

pub fn error(code: &str, message: &str) -> Value {
    json!({ "error": code, "message": message })
}

pub fn write_error(
    out: &mut impl Write,
    status: u16,
    code: &str,
    message: &str,
) -> std::io::Result<()> {
    write_json(out, status, &error(code, message))
}

pub fn call(
    stream: &mut (impl Read + Write),
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Response> {
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: vault\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    read_response(stream)
}

struct Input<'a, S: Read> {
    stream: &'a mut S,
    buf: Vec<u8>,
    pos: usize,
}

impl<S: Read> Input<'_, S> {
    fn fill(&mut self) -> Result<bool> {
        let mut chunk = [0u8; 8192];
        let read = self.stream.read(&mut chunk)?;
        self.buf.extend_from_slice(&chunk[..read]);
        Ok(read > 0)
    }

    fn head(&mut self) -> Result<usize> {
        loop {
            if let Some(at) = self.buf.windows(4).position(|w| w == b"\r\n\r\n") {
                self.pos = at + 4;
                return Ok(self.pos);
            }
            if self.buf.len() > MAX_HEAD {
                bail!("the request head is too long");
            }
            if !self.fill()? {
                bail!("the connection closed before the head ended");
            }
        }
    }

    fn line(&mut self) -> Result<String> {
        loop {
            if let Some(at) = self.buf[self.pos..].windows(2).position(|w| w == b"\r\n") {
                let line = String::from_utf8_lossy(&self.buf[self.pos..self.pos + at]).into_owned();
                self.pos += at + 2;
                return Ok(line);
            }
            if self.buf.len() - self.pos > MAX_HEAD || !self.fill()? {
                bail!("a bad chunk line");
            }
        }
    }

    fn take(&mut self, len: usize) -> Result<Vec<u8>> {
        while self.buf.len() - self.pos < len {
            if !self.fill()? {
                bail!("the connection closed inside the body");
            }
        }
        let out = self.buf[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Ok(out)
    }

    fn rest(&mut self) -> Result<Vec<u8>> {
        while self.fill()? {
            if self.buf.len() - self.pos > MAX_BODY {
                bail!("the body is too large");
            }
        }
        Ok(self.buf[self.pos..].to_vec())
    }

    fn body(&mut self, headers: &[(String, String)], until_close: bool) -> Result<Vec<u8>> {
        let chunked = header(headers, "transfer-encoding")
            .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
        if chunked {
            return self.chunked();
        }
        match header(headers, "content-length") {
            Some(len) => {
                let len: usize = len.trim().parse().context("a bad content-length")?;
                if len > MAX_BODY {
                    bail!("the body is too large");
                }
                self.take(len)
            }
            None if until_close => self.rest(),
            None => Ok(Vec::new()),
        }
    }

    fn chunked(&mut self) -> Result<Vec<u8>> {
        let mut body = Vec::new();
        loop {
            let line = self.line()?;
            let size = line.split(';').next().unwrap_or("").trim();
            let size = usize::from_str_radix(size, 16).context("a bad chunk size")?;
            if body.len() + size > MAX_BODY {
                bail!("the body is too large");
            }
            if size == 0 {
                while !self.line()?.is_empty() {}
                return Ok(body);
            }
            body.extend(self.take(size)?);
            self.take(2)?;
        }
    }
}

fn owned(headers: &[httparse::Header]) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|h| {
            (
                h.name.to_string(),
                String::from_utf8_lossy(h.value).into_owned(),
            )
        })
        .collect()
}

pub fn read_request(stream: &mut impl Read) -> Result<Request> {
    let mut input = Input {
        stream,
        buf: Vec::new(),
        pos: 0,
    };
    let end = input.head()?;
    let mut slots = [httparse::EMPTY_HEADER; 128];
    let mut parsed = httparse::Request::new(&mut slots);
    parsed
        .parse(&input.buf[..end])
        .context("a bad request head")?;
    let method = parsed.method.unwrap_or_default().to_string();
    let target = parsed.path.unwrap_or_default().to_string();
    let headers = owned(parsed.headers);
    let body = input.body(&headers, false)?;
    Ok(Request {
        method,
        target,
        headers,
        body,
    })
}

pub fn read_response(stream: &mut impl Read) -> Result<Response> {
    let mut input = Input {
        stream,
        buf: Vec::new(),
        pos: 0,
    };
    let end = input.head()?;
    let mut slots = [httparse::EMPTY_HEADER; 128];
    let mut parsed = httparse::Response::new(&mut slots);
    parsed
        .parse(&input.buf[..end])
        .context("a bad response head")?;
    let status = parsed.code.unwrap_or_default();
    let headers = owned(parsed.headers);
    let body = input.body(&headers, true)?;
    Ok(Response {
        status,
        headers,
        body,
    })
}

pub fn reason(status: u16) -> &'static str {
    ureq::http::StatusCode::from_u16(status)
        .ok()
        .and_then(|code| code.canonical_reason())
        .unwrap_or("Status")
}

pub fn write_head(
    out: &mut impl Write,
    status: u16,
    headers: &[(String, String)],
) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {status} {}\r\n", reason(status));
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    out.write_all(head.as_bytes())
}

pub fn write_json(out: &mut impl Write, status: u16, value: &Value) -> std::io::Result<()> {
    Response::json(status, value).write_to(out)
}

pub fn write_chunk(out: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    out.write_all(format!("{:x}\r\n", bytes.len()).as_bytes())?;
    out.write_all(bytes)?;
    out.write_all(b"\r\n")?;
    out.flush()
}

pub fn end_chunks(out: &mut impl Write) -> std::io::Result<()> {
    out.write_all(b"0\r\n\r\n")?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_plain_and_chunked_bodies() {
        let mut plain: &[u8] = b"POST /keys/a HTTP/1.1\r\nHost: x\r\nContent-Length: 2\r\n\r\n{}";
        let request = read_request(&mut plain).unwrap();
        assert_eq!(
            (request.method.as_str(), request.target.as_str()),
            ("POST", "/keys/a")
        );
        assert_eq!(request.body, b"{}");
        let mut chunked: &[u8] =
            b"PUT / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2;x=1\r\nde\r\n0\r\n\r\n";
        assert_eq!(read_request(&mut chunked).unwrap().body, b"abcde");
        let mut response: &[u8] = b"HTTP/1.1 403 Forbidden\r\nX-A: 1\r\n\r\nuntil close";
        let response = read_response(&mut response).unwrap();
        assert_eq!(response.status, 403);
        assert_eq!(response.body, b"until close");
    }
}
