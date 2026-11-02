//! The small part of HTTP the web interface needs: read a request, write an answer, and understand
//! `Range` headers so big files can be fetched in pieces and resumed.

use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};

pub const MAX_LINE: usize = 8 * 1024;
pub const MAX_HEADERS: usize = 64;

pub struct Req {
    pub method: String,
    /// The path without the query, e.g. `/api/downloads`.
    pub path: String,
    pub query: HashMap<String, String>,
    /// Header names in lower case.
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Req {
    pub fn header(&self, name: &str) -> &str {
        self.headers.get(name).map(String::as_str).unwrap_or("")
    }

    /// The value of one cookie, if the browser sent it.
    pub fn cookie(&self, name: &str) -> Option<String> {
        self.header("cookie").split(';').filter_map(|c| c.trim().split_once('=')).find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
    }
}

#[derive(Debug, PartialEq)]
pub enum ReadError {
    /// The connection ended or timed out before a whole request arrived.
    Closed,
    TooLarge,
    Malformed,
}

impl From<io::Error> for ReadError {
    fn from(_: io::Error) -> Self {
        ReadError::Closed
    }
}

fn read_line(r: &mut impl BufRead) -> Result<String, ReadError> {
    let mut line = String::new();
    r.take(MAX_LINE as u64).read_line(&mut line)?;
    if line.is_empty() {
        return Err(ReadError::Closed);
    }
    if !line.ends_with('\n') {
        return Err(ReadError::TooLarge);
    }
    Ok(line)
}

pub fn read_request(r: &mut impl BufRead, max_body: usize) -> Result<Req, ReadError> {
    let line = read_line(r)?;
    let mut parts = line.split_whitespace();
    let (method, target) = match (parts.next(), parts.next()) {
        (Some(m), Some(t)) => (m.to_string(), t.to_string()),
        _ => return Err(ReadError::Malformed),
    };
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), parse_query(q)),
        None => (target, HashMap::new()),
    };
    let mut headers = HashMap::new();
    for _ in 0..MAX_HEADERS {
        let h = read_line(r)?;
        if h.trim().is_empty() {
            let len: usize = headers.get("content-length").and_then(|v: &String| v.parse().ok()).unwrap_or(0);
            if len > max_body {
                return Err(ReadError::TooLarge);
            }
            let mut body = vec![0u8; len];
            r.read_exact(&mut body)?;
            return Ok(Req { method, path: percent_decode(&path), query, headers, body });
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    Err(ReadError::TooLarge)
}

pub fn percent_decode(s: &str) -> String {
    crate::engine::percent_decode(s)
}

pub fn parse_query(q: &str) -> HashMap<String, String> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (percent_decode(&k.replace('+', " ")), percent_decode(&v.replace('+', " ")))
        })
        .collect()
}

/// What a `Range` header asks for, given the file's size.
#[derive(Debug, PartialEq)]
pub enum Span {
    /// No (usable) range: send everything.
    Whole,
    /// First and last byte, both included.
    Part(u64, u64),
    /// The range lies outside the file.
    Unsatisfiable,
}

pub fn parse_range(header: &str, len: u64) -> Span {
    let Some(spec) = header.trim().strip_prefix("bytes=") else { return Span::Whole };
    // Several ranges at once are legal to ignore.
    if spec.contains(',') {
        return Span::Whole;
    }
    let Some((a, b)) = spec.split_once('-') else { return Span::Whole };
    let (a, b) = (a.trim(), b.trim());
    if len == 0 {
        return Span::Unsatisfiable;
    }
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(start), Ok(end)) if start <= end => {
            if start >= len {
                Span::Unsatisfiable
            } else {
                Span::Part(start, end.min(len - 1))
            }
        }
        (Ok(start), Err(_)) if b.is_empty() => {
            if start >= len {
                Span::Unsatisfiable
            } else {
                Span::Part(start, len - 1)
            }
        }
        // The last N bytes.
        (Err(_), Ok(n)) if a.is_empty() && n > 0 => Span::Part(len.saturating_sub(n), len - 1),
        (Err(_), Ok(_)) if a.is_empty() => Span::Unsatisfiable,
        _ => Span::Whole,
    }
}

/// A file name safe to put in a `Content-Disposition` header.
pub fn disposition(name: &str) -> String {
    let ascii: String = name.chars().map(|c| if c.is_ascii_graphic() && !matches!(c, '"' | '\\' | '%' | ';') || c == ' ' { c } else { '_' }).collect();
    let mut encoded = String::new();
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

pub fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        416 => "Range Not Satisfiable",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

/// Write the status line and headers; the body follows.
pub fn write_head(w: &mut impl Write, code: u16, headers: &[(&str, String)]) -> io::Result<()> {
    write!(w, "HTTP/1.1 {code} {}\r\n", reason(code))?;
    for (k, v) in headers {
        // A header value with a line break would let a caller add headers of their own.
        debug_assert!(!v.contains(['\r', '\n']));
        write!(w, "{k}: {}\r\n", v.replace(['\r', '\n'], " "))?;
    }
    write!(w, "Connection: close\r\n\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse(text: &str) -> Result<Req, ReadError> {
        read_request(&mut Cursor::new(text.as_bytes().to_vec()), 1024)
    }

    #[test]
    fn a_request_is_read_with_query_headers_and_body() {
        let r = parse("POST /api/downloads?status=paused&q=a%20b+c HTTP/1.1\r\nHost: x\r\nCookie: a=1; sdm=tok\r\nContent-Length: 5\r\n\r\nhello").unwrap();
        assert_eq!((r.method.as_str(), r.path.as_str()), ("POST", "/api/downloads"));
        assert_eq!(r.query["status"], "paused");
        assert_eq!(r.query["q"], "a b c");
        assert_eq!(r.header("host"), "x");
        assert_eq!(r.cookie("sdm").as_deref(), Some("tok"));
        assert_eq!(r.cookie("nope"), None);
        assert_eq!(r.body, b"hello");
    }

    #[test]
    fn bad_requests_are_refused() {
        assert_eq!(parse("").err(), Some(ReadError::Closed));
        assert_eq!(parse("GET\r\n\r\n").err(), Some(ReadError::Malformed));
        assert_eq!(parse("POST / HTTP/1.1\r\nContent-Length: 5000\r\n\r\n").err(), Some(ReadError::TooLarge));
        let long = format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(MAX_LINE + 10));
        assert_eq!(parse(&long).err(), Some(ReadError::TooLarge));
        let many = format!("GET / HTTP/1.1\r\n{}\r\n", "X: y\r\n".repeat(MAX_HEADERS + 1));
        assert_eq!(parse(&many).err(), Some(ReadError::TooLarge));
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Span::Part(0, 99));
        assert_eq!(parse_range("bytes=900-", 1000), Span::Part(900, 999));
        assert_eq!(parse_range("bytes=-100", 1000), Span::Part(900, 999));
        assert_eq!(parse_range("bytes=990-5000", 1000), Span::Part(990, 999));
        assert_eq!(parse_range("bytes=1000-", 1000), Span::Unsatisfiable);
        assert_eq!(parse_range("bytes=-0", 1000), Span::Unsatisfiable);
        assert_eq!(parse_range("bytes=5-2", 1000), Span::Whole);
        assert_eq!(parse_range("bytes=0-1,5-9", 1000), Span::Whole);
        assert_eq!(parse_range("items=0-1", 1000), Span::Whole);
        assert_eq!(parse_range("", 1000), Span::Whole);
        assert_eq!(parse_range("bytes=0-", 0), Span::Unsatisfiable);
    }

    #[test]
    fn file_names_cannot_break_out_of_the_header() {
        let d = disposition("re\"port\r\nX: 1; é.pdf");
        assert!(!d.contains('\r') && !d.contains('\n'));
        assert!(d.starts_with("attachment; filename=\"re_port__X: 1_ _.pdf\""), "{d}");
        assert!(d.ends_with("filename*=UTF-8''re%22port%0D%0AX%3A%201%3B%20%C3%A9.pdf"), "{d}");
    }
}
