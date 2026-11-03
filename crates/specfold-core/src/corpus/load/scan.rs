//! What specfold extracts from one Markdown file. This is the unit the cache stores.

use crate::markdown::{body, frontmatter};
use crate::model::doc::Frontmatter;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileScan {
    pub frontmatter: Frontmatter,
    pub body: body::Body,
    pub body_hash: String,
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Splits frontmatter from body and scans the body for references and anchors.
pub fn scan(text: &str, cite_re: &Regex) -> FileScan {
    let split = frontmatter::split(text);
    let fm = match frontmatter::parse(split.raw) {
        frontmatter::Parsed::None => Frontmatter::default(),
        frontmatter::Parsed::Map(m) => Frontmatter { map: Some(m), error: None },
        frontmatter::Parsed::Invalid(e) => Frontmatter { map: None, error: Some(e) },
    };
    FileScan {
        frontmatter: fm,
        body: body::scan(split.body, split.body_line, cite_re),
        body_hash: hex(&Sha256::digest(split.body.as_bytes())),
    }
}
