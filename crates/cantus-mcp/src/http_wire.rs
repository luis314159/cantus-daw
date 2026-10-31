//! HTTP/1.1 as far as the door and the bridge need it (RFC 9112): a request or response
//! head, a body by `Content-Length` or `chunked`, `Expect: 100-continue`, keep-alive. On
//! `127.0.0.1` only; every size is bounded.

use std::io::{self, BufRead, Read, Write};

/// The longest head accepted.
pub const MAX_HEAD: usize = 16 << 10;

/// A message's head and body.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Message {
    /// Request: method and target. Response: status and reason.
    pub start: (String, String),
    /// Names lower-cased, in order.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Message {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Whether the connection should close after this message.
    pub fn closes(&self) -> bool {
        self.header("connection")
            .is_some_and(|c| c.eq_ignore_ascii_case("close"))
    }
}

/// Why a message could not be read.
#[derive(Debug)]
pub enum WireError {
    /// The peer closed the connection before a message began.
    Closed,
    Io(io::Error),
    /// Malformed or over a limit: answer `status` and close.
    Bad(u16, &'static str),
}

impl From<io::Error> for WireError {
    fn from(e: io::Error) -> Self {
        WireError::Io(e)
    }
}

fn line(r: &mut impl BufRead, budget: &mut usize) -> Result<String, WireError> {
    let mut buf = Vec::new();
    let n = r
        .by_ref()
        .take(*budget as u64 + 1)
        .read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Err(WireError::Closed);
    }
    if n > *budget {
        return Err(WireError::Bad(431, "the head is too large"));
    }
    *budget -= n;
    while buf.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
        buf.pop();
    }
    String::from_utf8(buf).map_err(|_| WireError::Bad(400, "the head is not UTF-8"))
}

/// Read one message. `max_body` bounds the body (413 beyond); `continue_to` is written to
/// before reading the body when the peer asked `Expect: 100-continue` (the door's side).
pub fn read(
    r: &mut impl BufRead,
    max_body: usize,
    continue_to: Option<&mut dyn Write>,
) -> Result<Message, WireError> {
    let mut m = read_head(r)?;
    read_body(r, &mut m, max_body, continue_to)?;
    Ok(m)
}

/// Read a message's head only: the door checks who is asking before it reads a byte of
/// the body (review 3 #3).
pub fn read_head(r: &mut impl BufRead) -> Result<Message, WireError> {
    let mut budget = MAX_HEAD;
    let mut first = line(r, &mut budget)?;
    // Tolerate empty lines before a message (RFC 9112 §2.2).
    while first.is_empty() {
        first = line(r, &mut budget)?;
    }
    let mut parts = first.splitn(3, ' ');
    let (a, b) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let c = parts.next().unwrap_or("");
    let start = if a.starts_with("HTTP/") {
        (b.to_string(), c.to_string())
    } else {
        if !c.starts_with("HTTP/1.") || a.is_empty() || b.is_empty() {
            return Err(WireError::Bad(400, "not an HTTP/1.1 request line"));
        }
        (a.to_string(), b.to_string())
    };
    let mut headers = Vec::new();
    loop {
        let l = line(r, &mut budget)?;
        if l.is_empty() {
            break;
        }
        let Some((n, v)) = l.split_once(':') else {
            return Err(WireError::Bad(400, "a header without a colon"));
        };
        headers.push((n.trim().to_ascii_lowercase(), v.trim().to_string()));
        if headers.len() > 100 {
            return Err(WireError::Bad(431, "too many headers"));
        }
    }
    Ok(Message {
        start,
        headers,
        body: Vec::new(),
    })
}

/// Read the body `m`'s head announces.
pub fn read_body(
    r: &mut impl BufRead,
    m: &mut Message,
    max_body: usize,
    continue_to: Option<&mut dyn Write>,
) -> Result<(), WireError> {
    let chunked = m
        .header("transfer-encoding")
        .is_some_and(|t| t.to_ascii_lowercase().contains("chunked"));
    let length = match m.header("content-length") {
        Some(l) => Some(
            l.parse::<usize>()
                .map_err(|_| WireError::Bad(400, "a bad Content-Length"))?,
        ),
        None => None,
    };
    if length.is_some_and(|l| l > max_body) {
        return Err(WireError::Bad(413, "the body is too large"));
    }
    if (chunked || length.is_some_and(|l| l > 0))
        && m.header("expect")
            .is_some_and(|e| e.eq_ignore_ascii_case("100-continue"))
    {
        if let Some(w) = continue_to {
            w.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
            w.flush()?;
        }
    }
    if chunked {
        loop {
            let size_line = line(r, &mut 1024)?;
            let size = usize::from_str_radix(size_line.split(';').next().unwrap_or("").trim(), 16)
                .map_err(|_| WireError::Bad(400, "a bad chunk size"))?;
            if size == 0 {
                // Trailers, then the empty line.
                let mut b = MAX_HEAD;
                while !line(r, &mut b)?.is_empty() {}
                break;
            }
            // Never `len + size`: a size of 2⁶⁴−1 would wrap past the check (review 3 #2).
            if size > max_body.saturating_sub(m.body.len()) {
                return Err(WireError::Bad(413, "the body is too large"));
            }
            let at = m.body.len();
            m.body.resize(at + size, 0);
            r.read_exact(&mut m.body[at..])?;
            let mut crlf = [0u8; 2];
            r.read_exact(&mut crlf)?;
        }
    } else if let Some(l) = length {
        m.body.resize(l, 0);
        r.read_exact(&mut m.body)?;
    }
    Ok(())
}

/// Write a response.
pub fn respond(
    w: &mut impl Write,
    status: u16,
    headers: &[(&str, &str)],
    body: &[u8],
) -> io::Result<()> {
    let mut head = format!("HTTP/1.1 {status} {}\r\n", reason(status));
    for (n, v) in headers {
        head.push_str(&format!("{n}: {v}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    w.write_all(head.as_bytes())?;
    w.write_all(body)?;
    w.flush()
}

/// Write a request.
pub fn request(
    w: &mut impl Write,
    method: &str,
    target: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> io::Result<()> {
    let mut head = format!("{method} {target} HTTP/1.1\r\n");
    for (n, v) in headers {
        head.push_str(&format!("{n}: {v}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    w.write_all(head.as_bytes())?;
    w.write_all(body)?;
    w.flush()
}

pub fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    #[test]
    fn a_request_by_length_and_by_chunks() {
        let raw = b"POST /mcp HTTP/1.1\r\nHost: x\r\nContent-Length: 2\r\n\r\n{}";
        let m = read(&mut BufReader::new(&raw[..]), 100, None).unwrap();
        assert_eq!(m.start, ("POST".into(), "/mcp".into()));
        assert_eq!(m.header("host"), Some("x"));
        assert_eq!(m.body, b"{}");
        let raw = b"POST /mcp HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{\"\r\n3\r\na\":\r\n2\r\n1}\r\n0\r\n\r\n";
        let m = read(&mut BufReader::new(&raw[..]), 100, None).unwrap();
        assert_eq!(m.body, b"{\"a\":1}");
    }

    #[test]
    fn limits_and_malformed_heads_are_refused() {
        let raw = b"POST /mcp HTTP/1.1\r\nContent-Length: 200\r\n\r\n";
        assert!(matches!(
            read(&mut BufReader::new(&raw[..]), 100, None),
            Err(WireError::Bad(413, _))
        ));
        let raw = b"garbage\r\n\r\n";
        assert!(matches!(
            read(&mut BufReader::new(&raw[..]), 100, None),
            Err(WireError::Bad(400, _))
        ));
        let mut big = b"GET / HTTP/1.1\r\nX: ".to_vec();
        big.extend(std::iter::repeat_n(b'a', MAX_HEAD));
        big.extend(b"\r\n\r\n");
        assert!(matches!(
            read(&mut BufReader::new(&big[..]), 100, None),
            Err(WireError::Bad(431, _))
        ));
        assert!(matches!(
            read(&mut BufReader::new(&b""[..]), 100, None),
            Err(WireError::Closed)
        ));
    }

    /// Review 3 #2: a chunk size near 2⁶⁴ is refused, never wrapped past the limit.
    #[test]
    fn a_huge_chunk_size_is_refused_not_wrapped() {
        let raw = b"POST /mcp HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\nffffffffffffffff\r\nx";
        assert!(matches!(
            read(&mut BufReader::new(&raw[..]), 100, None),
            Err(WireError::Bad(413, _))
        ));
    }

    #[test]
    fn expect_continue_is_answered_before_the_body() {
        let raw = b"POST /mcp HTTP/1.1\r\nExpect: 100-continue\r\nContent-Length: 2\r\n\r\n{}";
        let mut out = Vec::new();
        let m = read(&mut BufReader::new(&raw[..]), 100, Some(&mut out)).unwrap();
        assert_eq!(m.body, b"{}");
        assert_eq!(out, b"HTTP/1.1 100 Continue\r\n\r\n");
    }

    #[test]
    fn a_response_round_trips() {
        let mut out = Vec::new();
        respond(&mut out, 202, &[("X-A", "b")], b"").unwrap();
        let m = read(&mut BufReader::new(&out[..]), 100, None).unwrap();
        assert_eq!(m.start.0, "202");
        assert_eq!(m.header("x-a"), Some("b"));
    }
}
