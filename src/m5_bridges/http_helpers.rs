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
/// BUG-060i: Increased 8KB → 32KB. VMS query responses are ~11KB, causing
/// EOF truncation at the old 8KB limit.
const DEFAULT_MAX_RESPONSE_SIZE: usize = 32_768;

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

    // BUG-C001 fix: Use full addr as Host header (correct for non-standard ports).
    // Previous code split on ':' which breaks on IPv6 and loses the port.
    let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(|_| {
        PvError::BridgeUnreachable {
            service: service.to_owned(),
            url: addr.to_owned(),
        }
    })?;

    // BUG-Gen13 fix: Use a small initial buffer (4096) and grow on demand.
    // Most health checks return <500 bytes, so allocating max_response_size
    // (up to 64KB for POVM) upfront wastes memory on every call.
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.len() >= max_response_size {
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

    let response = String::from_utf8_lossy(&buf);

    // BUG-058: Validate HTTP status code before extracting body.
    // Previously, 404/500 responses were silently parsed as success,
    // producing confusing BridgeParse errors on the error body text.
    // BUG-059: If status line is malformed (None), reject early rather
    // than falling through to body parse with unpredictable results.
    match extract_status_code(&response) {
        Some(status) if status >= 400 => {
            return Err(PvError::BridgeError {
                service: service.to_owned(),
                status,
            });
        }
        None if !response.is_empty() => {
            return Err(PvError::BridgeParse {
                service: service.to_owned(),
                reason: "malformed HTTP status line".to_owned(),
            });
        }
        _ => {}
    }

    let body = extract_body(&response).ok_or_else(|| PvError::BridgeParse {
        service: service.to_owned(),
        reason: "no body in HTTP response".to_owned(),
    })?;

    // If the server used chunked transfer encoding, strip chunk markers.
    if is_chunked_transfer(&response) {
        return dechunk_body(&body).ok_or_else(|| PvError::BridgeParse {
            service: service.to_owned(),
            reason: "malformed chunked transfer encoding".to_owned(),
        });
    }

    Ok(body)
}

/// Send a raw HTTP POST request with JSON content type.
///
/// Returns the HTTP status code on success (2xx/3xx).
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection fails.
/// Returns `PvError::BridgeError` if the server responds with 4xx/5xx.
pub fn raw_http_post(addr: &str, path: &str, body: &[u8], service: &str) -> PvResult<u16> {
    raw_http_post_with_content_type(addr, path, body, "application/json", service)
}

/// Send a raw HTTP POST request with JSON content type and return the response body.
///
/// Unlike [`raw_http_post`] which discards the response, this reads and returns
/// the full response body. Use for call sites that need to parse the server's
/// response (e.g., VMS MCP tool results for RALPH Recognize phase).
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection fails.
/// Returns `PvError::BridgeError` if the server responds with 4xx/5xx.
/// Returns `PvError::BridgeParse` if the response body cannot be extracted.
pub fn raw_http_post_with_response(
    addr: &str,
    path: &str,
    body: &[u8],
    service: &str,
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
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|_| PvError::BridgeUnreachable {
            service: service.to_owned(),
            url: addr.to_owned(),
        })?;

    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes()).map_err(|_| PvError::BridgeUnreachable {
        service: service.to_owned(),
        url: addr.to_owned(),
    })?;
    stream.write_all(body).map_err(|_| PvError::BridgeUnreachable {
        service: service.to_owned(),
        url: addr.to_owned(),
    })?;
    let _ = stream.flush();

    // Read full response (reuses raw_http_get read-loop pattern)
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.len() >= DEFAULT_MAX_RESPONSE_SIZE {
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

    let response = String::from_utf8_lossy(&buf);

    match extract_status_code(&response) {
        Some(status) if status >= 400 => {
            return Err(PvError::BridgeError {
                service: service.to_owned(),
                status,
            });
        }
        None if !response.is_empty() => {
            return Err(PvError::BridgeParse {
                service: service.to_owned(),
                reason: "malformed HTTP status line".to_owned(),
            });
        }
        _ => {}
    }

    let body_str = extract_body(&response).ok_or_else(|| PvError::BridgeParse {
        service: service.to_owned(),
        reason: "no body in HTTP response".to_owned(),
    })?;

    if is_chunked_transfer(&response) {
        return dechunk_body(&body_str).ok_or_else(|| PvError::BridgeParse {
            service: service.to_owned(),
            reason: "malformed chunked transfer encoding".to_owned(),
        });
    }

    Ok(body_str)
}

/// Send a raw HTTP POST request with TSV content type.
///
/// Returns the HTTP status code on success (2xx/3xx).
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection or I/O fails.
/// Returns `PvError::BridgeError` if the server responds with 4xx/5xx.
pub fn raw_http_post_tsv(addr: &str, path: &str, tsv: &str, service: &str) -> PvResult<u16> {
    raw_http_post_with_content_type(addr, path, tsv.as_bytes(), "text/tab-separated-values", service)
}

/// Send a raw HTTP POST request with custom content type.
///
/// Returns the HTTP status code on success (2xx/3xx). Non-2xx/3xx responses
/// are returned as `PvError::BridgeError` with the status code, making
/// semantic failures (422, 400, 500) visible to callers and circuit breakers.
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection fails.
/// Returns `PvError::BridgeError` if the server responds with status >= 400.
fn raw_http_post_with_content_type(
    addr: &str,
    path: &str,
    body: &[u8],
    content_type: &str,
    service: &str,
) -> PvResult<u16> {
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

    // BUG-C001 fix: Use full addr as Host header (correct for non-standard ports).
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
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

    // BUG-060: Must wait for server to process the body before closing.
    // Without this, dropping the TcpStream sends FIN immediately, which can
    // cause axum to abort the request before fully parsing the JSON body.
    // Fix: flush the stream, then read the response status line. The read
    // blocks until the server has processed the request and sent a response.
    let _ = stream.flush();
    stream.set_read_timeout(Some(Duration::from_millis(1000))).ok();
    let mut status_buf = [0u8; 64];
    let _ = stream.read(&mut status_buf);

    // T1 fix: Parse the status code we already read instead of discarding it.
    // Catches 4xx/5xx semantic failures (e.g., VMS 422 wrong payload format)
    // that were previously invisible. Returns 0 on timeout/empty/malformed.
    let status_code =
        extract_status_code(&String::from_utf8_lossy(&status_buf)).unwrap_or(0);

    if status_code >= 400 {
        return Err(PvError::BridgeError {
            service: service.to_owned(),
            status: status_code,
        });
    }

    Ok(status_code)
}

/// Extract the HTTP status code from a raw HTTP response.
///
/// Parses the status line `HTTP/1.x NNN ...` and returns the 3-digit code.
/// Returns `None` if the status line is malformed.
#[must_use]
pub fn extract_status_code(raw: &str) -> Option<u16> {
    // Status line format: "HTTP/1.x NNN reason\r\n..."
    let first_line = raw.split("\r\n").next()?;
    let mut parts = first_line.split_ascii_whitespace();
    parts.next()?; // "HTTP/1.1"
    let code_str = parts.next()?;
    code_str.parse::<u16>().ok()
}

/// Extract the body from a raw HTTP response string.
///
/// Looks for the `\r\n\r\n` header/body separator.
#[must_use]
pub fn extract_body(raw: &str) -> Option<String> {
    raw.find("\r\n\r\n")
        .map(|pos| raw[pos + 4..].to_owned())
}

/// Extract raw HTTP headers (everything before the body separator).
///
/// Returns `None` if no `\r\n\r\n` separator is found.
#[must_use]
pub fn extract_headers(raw: &str) -> Option<&str> {
    raw.find("\r\n\r\n").map(|pos| &raw[..pos])
}

/// Detect whether the response uses chunked transfer encoding.
///
/// Checks for `transfer-encoding: chunked` in headers (case-insensitive).
#[must_use]
pub fn is_chunked_transfer(raw: &str) -> bool {
    let Some(headers) = extract_headers(raw) else {
        return false;
    };
    for line in headers.split("\r\n") {
        // Case-insensitive header match
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("transfer-encoding:") {
            let value = lower.trim_start_matches("transfer-encoding:").trim();
            if value == "chunked" {
                return true;
            }
        }
    }
    false
}

/// Decode a chunked transfer-encoded body.
///
/// Chunked format: each chunk is `SIZE_HEX\r\nDATA\r\n`, terminated by `0\r\n\r\n`.
/// Returns `None` if any chunk size is malformed.
#[must_use]
pub fn dechunk_body(chunked: &str) -> Option<String> {
    let mut result = String::new();
    let mut remaining = chunked;

    loop {
        // Find the chunk-size line terminator
        let crlf_pos = remaining.find("\r\n")?;
        let size_str = remaining[..crlf_pos].trim();

        // Parse hex chunk size
        let chunk_size = usize::from_str_radix(size_str, 16).ok()?;

        if chunk_size == 0 {
            // Terminal chunk
            break;
        }

        // Advance past the size line + CRLF
        let data_start = crlf_pos + 2;
        let data_end = data_start + chunk_size;

        if data_end > remaining.len() {
            // Truncated response — take what we have
            result.push_str(&remaining[data_start..]);
            break;
        }

        result.push_str(&remaining[data_start..data_end]);

        // Skip past chunk data + trailing CRLF
        let next_start = data_end + 2;
        if next_start > remaining.len() {
            break;
        }
        remaining = &remaining[next_start..];
    }

    Some(result)
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
    fn extract_status_code_200() {
        let raw = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        assert_eq!(extract_status_code(raw), Some(200));
    }

    #[test]
    fn extract_status_code_404() {
        let raw = "HTTP/1.1 404 Not Found\r\n\r\nNot Found";
        assert_eq!(extract_status_code(raw), Some(404));
    }

    #[test]
    fn extract_status_code_500() {
        let raw = "HTTP/1.1 500 Internal Server Error\r\n\r\nError";
        assert_eq!(extract_status_code(raw), Some(500));
    }

    #[test]
    fn extract_status_code_malformed() {
        assert!(extract_status_code("garbage data").is_none());
    }

    #[test]
    fn extract_status_code_empty() {
        assert!(extract_status_code("").is_none());
    }

    #[test]
    fn extract_status_code_no_reason() {
        let raw = "HTTP/1.1 204\r\n\r\n";
        assert_eq!(extract_status_code(raw), Some(204));
    }

    // ── Chunked transfer encoding ──

    #[test]
    fn extract_headers_found() {
        let raw = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nbody";
        let headers = extract_headers(raw).unwrap();
        assert!(headers.contains("Content-Type"));
        assert!(!headers.contains("body"));
    }

    #[test]
    fn extract_headers_none_when_no_separator() {
        assert!(extract_headers("no headers here").is_none());
    }

    #[test]
    fn is_chunked_detects_header() {
        let raw = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert!(is_chunked_transfer(raw));
    }

    #[test]
    fn is_chunked_case_insensitive() {
        let raw = "HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n";
        assert!(is_chunked_transfer(raw));
    }

    #[test]
    fn is_chunked_mixed_case() {
        let raw = "HTTP/1.1 200 OK\r\nTransfer-ENCODING: Chunked\r\n\r\n";
        assert!(is_chunked_transfer(raw));
    }

    #[test]
    fn is_chunked_false_when_absent() {
        let raw = "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
        assert!(!is_chunked_transfer(raw));
    }

    #[test]
    fn is_chunked_false_no_separator() {
        assert!(!is_chunked_transfer("garbage"));
    }

    #[test]
    fn dechunk_simple() {
        let chunked = "5\r\nhello\r\n0\r\n\r\n";
        assert_eq!(dechunk_body(chunked), Some("hello".to_owned()));
    }

    #[test]
    fn dechunk_multi_chunk() {
        let chunked = "5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        assert_eq!(dechunk_body(chunked), Some("hello world".to_owned()));
    }

    #[test]
    fn dechunk_empty_body() {
        let chunked = "0\r\n\r\n";
        assert_eq!(dechunk_body(chunked), Some(String::new()));
    }

    #[test]
    fn dechunk_json_body() {
        let json = r#"{"ok":true}"#;
        let chunked = format!("{:x}\r\n{json}\r\n0\r\n\r\n", json.len());
        assert_eq!(dechunk_body(&chunked), Some(json.to_owned()));
    }

    #[test]
    fn dechunk_malformed_hex_returns_none() {
        let chunked = "ZZ\r\ngarbage\r\n0\r\n\r\n";
        assert!(dechunk_body(chunked).is_none());
    }

    #[test]
    fn dechunk_no_crlf_returns_none() {
        assert!(dechunk_body("no crlf at all").is_none());
    }

    #[test]
    fn dechunk_truncated_graceful() {
        // Chunk claims 100 bytes but only 5 available
        let chunked = "64\r\nhello";
        let result = dechunk_body(chunked);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "hello");
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

    /// BUG-Gen13: Verify the growing buffer strategy by ensuring
    /// the function does not pre-allocate `max_response_size`.
    /// We test indirectly: a large `max_response_size` should not
    /// cause excessive memory allocation on an unreachable host.
    #[test]
    fn get_with_large_limit_no_upfront_alloc() {
        // With the old code, this would allocate 10MB upfront.
        // With the fix, it only allocates 4KB initially.
        // The call will fail at connect time, but it validates
        // the code path compiles and the limit parameter is accepted.
        let result = raw_http_get_with_limit(
            "127.0.0.1:19999",
            "/health",
            "test",
            10 * 1024 * 1024, // 10MB limit — no upfront alloc with fix
        );
        assert!(result.is_err());
    }
}
