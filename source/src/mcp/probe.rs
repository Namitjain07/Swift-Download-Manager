//! `probe_link`: what is at a link (does it answer, how big is it, what is it called, can it be resumed) without
//! downloading it, so an assistant can look before it adds something.
//!
//! The `--mcp` process does this itself and not the window, so a slow server never holds up the window. The
//! connection goes through the same guard as downloads do: names are looked up and addresses on private networks
//! dropped at the moment of connecting, which also covers redirects and names that change their answer.

use super::error::{ErrorKind, ToolError};
use super::policy::{untrusted, Policy};
use crate::engine;
use serde_json::{json, Value};
use std::time::Duration;
use ureq::ResponseExt;

const HOW_LONG: Duration = Duration::from_secs(10);

/// Look at `link` if the rules in `policy` allow it.
pub fn probe(link: &str, policy: &Policy) -> Result<Value, ToolError> {
    policy.check_link(link)?;
    let mut out = look(link.trim(), policy.block_private)?;
    // A redirect may lead somewhere the rules do not allow: say so, so the assistant does not add that address.
    let final_link = out["final_url"].as_str().unwrap_or("").to_string();
    if let Err(e) = policy.check_link(&final_link) {
        out["allowed"] = json!(false);
        out["not_allowed_because"] = json!(e.message);
    } else {
        out["allowed"] = json!(true);
    }
    Ok(out)
}

/// `https://host/path` of an address: no password, no query (it can hold a key), no fragment.
fn plain(link: &str) -> String {
    match url::Url::parse(link) {
        Ok(u) => format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or("?"), u.path()),
        Err(_) => String::new(),
    }
}

fn header(resp: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    resp.headers().get(name).and_then(|v| v.to_str().ok()).map(|s| untrusted(s, 300)).filter(|s| !s.is_empty())
}

fn failure(e: ureq::Error) -> ToolError {
    match e {
        ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => ToolError::forbidden("That address is on a private network or on this computer, which assistants may not reach.").hint("The person can allow this in Options > AI assistants."),
        ureq::Error::Timeout(_) => ToolError::new(ErrorKind::Unreachable, "The server did not answer in time.").hint("Try again later, or add the link anyway."),
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => ToolError::new(ErrorKind::Unreachable, "The secure connection could not be set up (the certificate may be wrong or expired)."),
        other => ToolError::new(ErrorKind::Unreachable, format!("Could not reach the server: {}", untrusted(&other.to_string(), 200))),
    }
}

/// The request itself, with the guard against private addresses switched on or off.
fn look(link: &str, block_private: bool) -> Result<Value, ToolError> {
    let config = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(HOW_LONG)).max_redirects(5).user_agent("SwiftDownloadManager/1.0").build();
    let agent = crate::netroute::agent(config, None, block_private);
    // A HEAD request first. Many servers refuse it; then one byte is asked for, which also shows whether ranges work.
    let mut resp = agent.head(link).call().map_err(failure)?;
    let mut ranged = false;
    if !resp.status().is_success() {
        let second = agent.get(link).header("Range", "bytes=0-0").call().map_err(failure)?;
        if second.status().is_success() {
            resp = second;
            ranged = true;
        }
    }
    let status = resp.status().as_u16();
    let size = header(&resp, "content-range").and_then(|v| v.rsplit('/').next().and_then(|t| t.trim().parse::<u64>().ok())).or_else(|| if ranged && status != 206 { None } else { header(&resp, "content-length").and_then(|v| v.parse().ok()) });
    let resumable = status == 206 || header(&resp, "accept-ranges").is_some_and(|v| v.eq_ignore_ascii_case("bytes"));
    let final_link = plain(&resp.get_uri().to_string());
    let name = header(&resp, "content-disposition").and_then(|v| engine::name_from_disposition(&v)).map(|n| engine::sanitize(&n)).unwrap_or_else(|| engine::file_name_from_url(link));
    let mut out = json!({
        "url": plain(link),
        "reachable": true,
        "status": status,
        "final_url": final_link,
        "redirected": final_link != plain(link),
        "size_bytes": size,
        "resumable": resumable,
        "content_type": header(&resp, "content-type"),
        "file_name": untrusted(&name, 200),
        "last_modified": header(&resp, "last-modified"),
    });
    if status >= 400 {
        out["note"] = json!(format!("The server answered {status}: the file is probably not there, or needs a login."));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A server that answers every request with `answer(first line of the request)`.
    fn serve(answer: fn(&str) -> String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut s = stream;
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let first = String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("").to_string();
                let _ = s.write_all(answer(&first).as_bytes());
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    fn open_policy() -> Policy {
        Policy { block_private: false, ..Policy::default() }
    }

    #[test]
    fn describes_a_file_without_downloading_it() {
        let base = serve(|first| {
            assert!(first.starts_with("HEAD "), "a HEAD request first: {first}");
            "HTTP/1.1 200 OK\r\nContent-Length: 1234567\r\nAccept-Ranges: bytes\r\nContent-Type: application/zip\r\nContent-Disposition: attachment; filename=\"nice.zip\"\r\nLast-Modified: Tue, 03 Nov 2026 10:00:00 GMT\r\nConnection: close\r\n\r\n".to_string()
        });
        let out = probe(&format!("{base}/download?id=7&token=SECRET"), &open_policy()).unwrap();
        assert_eq!(out["status"], 200);
        assert_eq!(out["size_bytes"], 1_234_567);
        assert_eq!(out["resumable"], true);
        assert_eq!(out["content_type"], "application/zip");
        assert_eq!(out["file_name"], "nice.zip");
        assert_eq!(out["redirected"], false);
        assert_eq!(out["allowed"], true);
        assert!(!out.to_string().contains("SECRET"), "the query of a link is never shown");
        assert!(out["url"].as_str().unwrap().ends_with("/download"));
    }

    #[test]
    fn falls_back_to_a_one_byte_range_when_head_is_refused() {
        let base = serve(|first| {
            if first.starts_with("HEAD ") {
                "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            } else {
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/9000\r\nContent-Length: 1\r\nConnection: close\r\n\r\nx".to_string()
            }
        });
        let out = probe(&format!("{base}/a.bin"), &open_policy()).unwrap();
        assert_eq!(out["status"], 206);
        assert_eq!(out["size_bytes"], 9000);
        assert_eq!(out["resumable"], true);
        assert_eq!(out["file_name"], "a.bin");
    }

    #[test]
    fn a_missing_file_is_a_result_not_an_error() {
        let base = serve(|_| "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string());
        let out = probe(&format!("{base}/gone"), &open_policy()).unwrap();
        assert_eq!(out["status"], 404);
        assert!(out["note"].as_str().unwrap().contains("404"));
    }

    #[test]
    fn private_addresses_are_refused_before_and_while_connecting() {
        let base = serve(|_| "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string());
        // The text of the link already says it is local.
        assert_eq!(probe(&format!("{base}/x"), &Policy::default()).unwrap_err().kind, ErrorKind::Forbidden);
        // A name the text does not give away is caught when the connection is made.
        let err = look(&format!("{base}/x"), true).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Forbidden, "{err}");
    }

    #[test]
    fn a_closed_port_is_unreachable() {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let e = probe(&format!("http://127.0.0.1:{port}/x"), &open_policy()).unwrap_err();
        assert_eq!(e.kind, ErrorKind::Unreachable, "{e}");
    }

    #[test]
    fn links_the_rules_forbid_are_not_touched() {
        let p = Policy { block_hosts: vec!["example.com".into()], ..open_policy() };
        assert_eq!(probe("https://example.com/a", &p).unwrap_err().kind, ErrorKind::Forbidden);
        assert_eq!(probe("ftp://example.com/a", &p).unwrap_err().kind, ErrorKind::InvalidArgument);
    }
}
