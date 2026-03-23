//! # Shared HTTP Helpers for Bridge Modules
//!
//! Raw TCP HTTP helpers extracted from M22-M25 to eliminate duplication (BUG-042).
//!
//! All bridges use raw `TcpStream` for minimal overhead — no HTTP library dependency.
//! Addresses MUST be raw `host:port` (no `http://` prefix, BUG-033).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::m1_core::m02_error_handling::{PvError, PvResult};

/// Default TCP connection timeout (milliseconds).
const DEFAULT_TCP_TIMEOUT_MS: u64 = 2000;

/// Default maximum response body size (bytes).
const DEFAULT_MAX_RESPONSE_SIZE: usize = 8192;

/// Send a raw HTTP GET request over TCP and return the response body.
///
/// Uses a raw `TcpStream` with configurable timeout and max response size.
/// The address must be `host:port` format (no `http://` prefix).
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection or I/O fails.
/// Returns `PvError::BridgeParse` if no HTTP body separator is found.
pub fn raw_http_get(addr: &str, path: &str, service: &str) -> PvResult<String> {
    raw_http_get_with_limit(addr, path, service, DEFAULT_MAX_RESPONSE_SIZE)
}

/// Send a raw HTTP GET request with a custom max response size.
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection or I/O fails.
/// Returns `PvError::BridgeParse` if no HTTP body separator is found.
pub fn raw_http_get_with_limit(
    addr: &str,
    path: &str,
    service: &str,
    max_response_size: usize,
) -> PvResult<String> {
    let timeout = Duration::from_millis(DEFAULT_TCP_TIMEOUT_MS);
    let mut stream = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|_| PvError::BridgeUnreachable {
                service: service.to_owned(),
                url: addr.to_owned(),
            })?,
        timeout,
    )
    .map_err(|_| PvError::BridgeUnreachable {
        service: service.to_owned(),
        url: addr.to_owned(),
    })?;

    stream
        .set_read_timeout(Some(timeout))
        .map_err(|_| PvError::BridgeUnreachable {
            service: service.to_owned(),
            url: addr.to_owned(),
        })?;

    let host = addr.split(':').next().unwrap_or("localhost");
    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(|_| {
        PvError::BridgeUnreachable {
            service: service.to_owned(),
            url: addr.to_owned(),
        }
    })?;

    let mut buf = vec![0u8; max_response_size];
    let mut total = 0;
    loop {
        match stream.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                if total >= max_response_size {
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) => {
                return Err(PvError::BridgeUnreachable {
                    service: service.to_owned(),
                    url: addr.to_owned(),
                });
            }
        }
    }

    let response = String::from_utf8_lossy(&buf[..total]);
    extract_body(&response).ok_or_else(|| PvError::BridgeParse {
        service: service.to_owned(),
        reason: "no body in HTTP response".to_owned(),
    })
}

/// Send a raw HTTP POST request with JSON content type (fire-and-forget).
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection fails.
pub fn raw_http_post(addr: &str, path: &str, body: &[u8], service: &str) -> PvResult<()> {
    raw_http_post_with_content_type(addr, path, body, "application/json", service)
}

/// Send a raw HTTP POST request with TSV content type (fire-and-forget).
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection or I/O fails.
pub fn raw_http_post_tsv(addr: &str, path: &str, tsv: &str, service: &str) -> PvResult<()> {
    raw_http_post_with_content_type(addr, path, tsv.as_bytes(), "text/tab-separated-values", service)
}

/// Send a raw HTTP POST request with custom content type (fire-and-forget).
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection fails.
fn raw_http_post_with_content_type(
    addr: &str,
    path: &str,
    body: &[u8],
    content_type: &str,
    service: &str,
) -> PvResult<()> {
    let timeout = Duration::from_millis(DEFAULT_TCP_TIMEOUT_MS);
    let mut stream = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|_| PvError::BridgeUnreachable {
                service: service.to_owned(),
                url: addr.to_owned(),
            })?,
        timeout,
    )
    .map_err(|_| PvError::BridgeUnreachable {
        service: service.to_owned(),
        url: addr.to_owned(),
    })?;

    // Set write timeout to prevent blocking the spawn_blocking thread pool
    // if the server accepts the connection but stalls on receiving data.
    stream.set_write_timeout(Some(timeout)).map_err(|_| PvError::BridgeUnreachable {
        service: service.to_owned(),
        url: addr.to_owned(),
    })?;

    let host = addr.split(':').next().unwrap_or("localhost");
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes()).map_err(|_| {
        PvError::BridgeUnreachable {
            service: service.to_owned(),
            url: addr.to_owned(),
        }
    })?;
    stream.write_all(body).map_err(|_| PvError::BridgeUnreachable {
        service: service.to_owned(),
        url: addr.to_owned(),
    })?;

    // Fire-and-forget: we don't wait for a response
    Ok(())
}

/// Extract the body from a raw HTTP response string.
///
/// Looks for the `\r\n\r\n` header/body separator.
#[must_use]
pub fn extract_body(raw: &str) -> Option<String> {
    raw.find("\r\n\r\n")
        .map(|pos| raw[pos + 4..].to_owned())
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_body_finds_body() {
        let raw = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        assert_eq!(extract_body(raw), Some("{\"ok\":true}".to_owned()));
    }

    #[test]
    fn extract_body_no_separator() {
        assert!(extract_body("just some text").is_none());
    }

    #[test]
    fn extract_body_empty_body() {
        let raw = "HTTP/1.1 204 No Content\r\n\r\n";
        assert_eq!(extract_body(raw), Some(String::new()));
    }

    #[test]
    fn extract_body_multiline_body() {
        let raw = "HTTP/1.1 200 OK\r\n\r\n{\"a\":1,\n\"b\":2}";
        assert_eq!(extract_body(raw), Some("{\"a\":1,\n\"b\":2}".to_owned()));
    }

    #[test]
    fn get_fails_unreachable() {
        let result = raw_http_get("127.0.0.1:19999", "/health", "test");
        assert!(result.is_err());
    }

    #[test]
    fn post_fails_unreachable() {
        let result = raw_http_post("127.0.0.1:19999", "/data", b"test", "test");
        assert!(result.is_err());
    }

    #[test]
    fn post_tsv_fails_unreachable() {
        let result = raw_http_post_tsv("127.0.0.1:19999", "/put", "a\tb\tc", "test");
        assert!(result.is_err());
    }

    #[test]
    fn get_with_limit_fails_unreachable() {
        let result = raw_http_get_with_limit("127.0.0.1:19999", "/health", "test", 65536);
        assert!(result.is_err());
    }
}
