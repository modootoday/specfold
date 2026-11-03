//! Content hashes of attachments.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// SHA-256 (hex) and size of a file, read in chunks.
pub fn file(path: &Path) -> std::io::Result<(String, u64)> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        total += n as u64;
    }
    Ok((h.finalize().iter().map(|b| format!("{b:02x}")).collect(), total))
}
