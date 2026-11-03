//! The wire: Content-Length framed JSON-RPC 2.0 messages, file URIs and UTF-16 positions.

use serde_json::Value;
use std::io::{BufRead, Write};
use std::path::PathBuf;

/// Largest message body accepted; a bigger Content-Length is treated as a broken stream.
const MAX_BODY: usize = 64 << 20;

fn broken(why: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, why)
}

/// One framed message (`Err` holds a JSON parse error for a -32700 reply), or None at the end
/// of input. A frame that cannot be delimited is an I/O error naming why.
pub fn read_message(r: &mut impl BufRead) -> std::io::Result<Option<Result<Value, String>>> {
    let mut len = None;
    let mut headers = false;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            if headers {
                return Err(broken("input ended inside message headers".into()));
            }
            return Ok(None);
        }
        let line = line.trim_end();
        if line.is_empty() {
            if headers {
                break;
            }
            continue;
        }
        headers = true;
        if let Some((k, v)) = line.split_once(':')
            && k.eq_ignore_ascii_case("content-length")
        {
            len = Some(v.trim().parse::<usize>().map_err(|_| broken(format!("bad Content-Length header: {line}")))?);
        }
    }
    let len = len.ok_or_else(|| broken("message headers have no Content-Length".into()))?;
    if len > MAX_BODY {
        return Err(broken(format!("Content-Length {len} exceeds the {MAX_BODY}-byte limit")));
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).map_err(|_| broken(format!("input ended inside a {len}-byte message body")))?;
    Ok(Some(serde_json::from_slice(&body).map_err(|e| e.to_string())))
}

pub fn write_message(w: &mut impl Write, v: &Value) -> std::io::Result<()> {
    let body = v.to_string();
    write!(w, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    w.flush()
}

/// `file:///a%20b/c.md` → `/a b/c.md`. Other schemes are not files.
pub fn uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let b = rest.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(n) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16)
        {
            out.push(n);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    let s = String::from_utf8(out).ok()?;
    // Windows: file:///C:/x → C:/x
    let s = if s.len() > 2 && s.as_bytes()[0] == b'/' && s.as_bytes()[2] == b':' { s[1..].to_string() } else { s };
    Some(PathBuf::from(s))
}

/// UTF-16 code units in `s` up to byte `b`: LSP positions count UTF-16 by default.
pub(super) fn utf16(s: &str, b: usize) -> usize {
    s[..b].encode_utf16().count()
}
