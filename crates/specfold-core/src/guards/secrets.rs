//! Built-in secret patterns. Deliberately narrow: each one is a format with a fixed prefix or
//! header, so a hit is almost always real. Two shapes need a second look: a connection string
//! (`scheme://user:password@host`) is skipped when its host is an example or loopback host or its
//! password is a placeholder, and a JWT is skipped when it is the well-known jwt.io sample.

use regex::Regex;
use std::sync::OnceLock;

pub struct SecretPattern {
    pub name: &'static str,
    pub re: Regex,
}

const PATTERNS: &[(&str, &str)] = &[
    ("private key", r"-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY(?: BLOCK)?-----"),
    ("AWS access key id", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
    ("GitHub token", r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{40,})\b"),
    ("GitLab token", r"\bglpat-[A-Za-z0-9_-]{20,}\b"),
    ("Slack token", r"\bxox[abposr]-[A-Za-z0-9-]{10,}\b"),
    ("Google API key", r"\bAIza[0-9A-Za-z_-]{35}\b"),
    ("Stripe live key", r"\b(?:sk|rk)_live_[0-9A-Za-z]{20,}\b"),
    ("npm token", r"\bnpm_[A-Za-z0-9]{36}\b"),
    ("Anthropic API key", r"\bsk-ant-[A-Za-z0-9_-]{20,}\b"),
    ("OpenAI API key", r"\bsk-(?:proj|svcacct|admin)-[A-Za-z0-9_-]{20,}\b"),
    ("Hugging Face token", r"\bhf_[A-Za-z]{34}\b"),
    (
        CONNECTION,
        r#"\b[A-Za-z][A-Za-z0-9+.-]{1,30}://([^\s:/?#@'"<>\[\]]+):([^\s/?#@'"<>]{4,})@(\[[0-9A-Fa-f:.]+\]|[^\s/?#@:'"<>]+)"#,
    ),
    ("JWT", r"\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{16,}"),
];

const CONNECTION: &str = "connection string with a password";

/// The signature of the sample token on jwt.io, pasted into countless docs.
const JWT_SAMPLE_SIGNATURE: &str = "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

/// Hosts no real credential points at: documentation domains (RFC 2606, RFC 6761) and loopback.
fn example_host(host: &str) -> bool {
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    let h = h.trim_start_matches('[').trim_end_matches(']');
    let under = |d: &str| h == d || h.ends_with(&format!(".{d}"));
    ["example.com", "example.org", "example.net", "example", "test", "invalid", "localhost"].iter().any(|d| under(d))
        || h.starts_with("127.")
        || h == "0.0.0.0"
        || h == "::1"
}

/// Passwords written to be replaced: `${VAR}`, `$VAR`, `%VAR%`, `{{var}}`, `<var>`, a run of one
/// character (`xxxx`, `****`), or a word that only names the slot.
fn placeholder(pw: &str, user: &str) -> bool {
    const WORDS: &[&str] = &[
        "password",
        "passwd",
        "pass",
        "pwd",
        "changeme",
        "change_me",
        "change-me",
        "secret",
        "example",
        "placeholder",
        "dummy",
        "redacted",
        "your_password",
        "yourpassword",
        "your-password",
        "mypassword",
        "my_password",
        "pass123",
    ];
    let l = pw.to_ascii_lowercase();
    let mut chars = pw.chars();
    let first = chars.next().unwrap_or(' ');
    pw.starts_with('$')
        || pw.starts_with("{{")
        || pw.starts_with('<')
        || (pw.starts_with('%') && pw.ends_with('%'))
        || chars.all(|c| c == first)
        || WORDS.contains(&l.as_str())
        || l == user.to_ascii_lowercase()
}

/// One built-in secret in a text, by byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub start: usize,
    pub end: usize,
    pub name: &'static str,
}

/// Every built-in secret in `text`, in order, without the example and placeholder shapes above.
pub fn find(text: &str) -> Vec<Hit> {
    let mut out = vec![];
    for p in all() {
        for c in p.re.captures_iter(text) {
            let m = c.get(0).expect("group 0 is the match");
            let keep = match p.name {
                CONNECTION => {
                    let (user, pw, host) = (&c[1], &c[2], &c[3]);
                    !example_host(host) && !placeholder(pw, user)
                }
                "JWT" => !m.as_str().ends_with(JWT_SAMPLE_SIGNATURE),
                _ => true,
            };
            if keep {
                out.push(Hit { start: m.start(), end: m.end(), name: p.name });
            }
        }
    }
    out.sort_by_key(|h| (h.start, std::cmp::Reverse(h.end)));
    out
}

/// `text` with each built-in secret replaced by `[redacted: <name>]`, and how many were replaced.
pub fn redact(text: &str) -> (String, usize) {
    let hits = find(text);
    if hits.is_empty() {
        return (text.to_string(), 0);
    }
    let (mut out, mut at, mut n) = (String::with_capacity(text.len()), 0, 0);
    for h in &hits {
        if h.start < at {
            continue;
        }
        out.push_str(&text[at..h.start]);
        out.push_str(&format!("{MARKER}{}]", h.name));
        at = h.end;
        n += 1;
    }
    out.push_str(&text[at..]);
    (out, n)
}

/// How a redacted value starts; a body holding it was read through redaction.
pub const MARKER: &str = "[redacted: ";

pub fn all() -> &'static [SecretPattern] {
    static P: OnceLock<Vec<SecretPattern>> = OnceLock::new();
    P.get_or_init(|| {
        PATTERNS.iter().filter_map(|(name, re)| Regex::new(re).ok().map(|re| SecretPattern { name, re })).collect()
    })
}

/// A document's repository attachments that the secrets check reads: bundle files and `files`
/// entries kept in the repository, at most `max` bytes, and text (no NUL in the first 8 KiB).
/// Stored files are not here; `spec attach` checks them before upload.
pub fn scanned_attachments(root: &std::path::Path, d: &crate::model::Doc, max: u64) -> Vec<String> {
    attachment_scan(root, d, max).0
}

/// The attachments the secrets check reads, and the ones it skips because they are over `max`
/// bytes or binary. Missing files are in neither list; E2 reports them.
pub fn attachment_scan(root: &std::path::Path, d: &crate::model::Doc, max: u64) -> (Vec<String>, Vec<String>) {
    let base = d.base_dir();
    let mut paths: Vec<String> = d.bundle_files.clone();
    for e in d.files().into_iter().filter(|e| e.store != "store") {
        let p = if base.is_empty() { e.path } else { format!("{base}/{}", e.path) };
        if !paths.contains(&p) {
            paths.push(p);
        }
    }
    let (mut read, mut skipped) = (vec![], vec![]);
    for p in paths {
        let full = root.join(&p);
        let Ok(meta) = std::fs::metadata(&full) else { continue };
        if !meta.is_file() {
            continue;
        }
        if meta.len() > max {
            skipped.push(p);
            continue;
        }
        let Ok(bytes) = std::fs::read(&full) else { continue };
        if bytes[..bytes.len().min(8192)].contains(&0) {
            skipped.push(p);
        } else {
            read.push(p);
        }
    }
    (read, skipped)
}

/// One line naming attachments the secrets check did not read, or None when it read them all.
pub fn skipped_note(skipped: &[String]) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    let mut names: Vec<&str> = skipped.iter().map(String::as_str).take(5).collect();
    let more = skipped.len().saturating_sub(5);
    let more = if more > 0 { format!(" (+{more} more)") } else { String::new() };
    names.sort_unstable();
    Some(format!(
        "{} attachment(s) not scanned for secrets (over [files] scan_max_bytes or binary): {}{more}; raise [files] scan_max_bytes in spec.toml to read larger text files",
        skipped.len(),
        names.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Samples are assembled at run time so this file never holds a string that looks like a
    // real credential to secret scanners.
    #[test]
    fn detects_known_formats_only() {
        let hit = |s: &str| all().iter().find(|p| p.re.is_match(s)).map(|p| p.name);
        assert_eq!(hit(&format!("-----BEGIN OPENSSH {}-----", "PRIVATE KEY")), Some("private key"));
        assert_eq!(hit(&format!("key {}{} here", "AK", "IA".to_string() + &"X".repeat(16))), Some("AWS access key id"));
        assert_eq!(hit(&format!("{}_{}", "ghp", "a".repeat(36))), Some("GitHub token"));
        assert_eq!(hit("the word secret and sk_test_123"), None);
    }

    fn names(s: &str) -> Vec<&'static str> {
        find(s).into_iter().map(|h| h.name).collect()
    }

    #[test]
    fn connection_strings_with_real_hosts_hit() {
        let pw = format!("{}{}", "s3cr", "Et-77x");
        for scheme in ["postgres", "postgresql", "mysql", "mongodb+srv", "redis", "amqp"] {
            let s = format!("url = \"{scheme}://app:{pw}@db.internal.acme.io:5432/main\"");
            assert_eq!(names(&s), [CONNECTION], "{s}");
        }
        assert_eq!(names(&format!("redis://default:{pw}@10.0.4.2:6379")), [CONNECTION]);
    }

    #[test]
    fn connection_string_false_positives_pass() {
        let must_pass = [
            "postgres://app:hunter2x@example.com/db",
            "postgres://app:hunter2x@db.example.org/db",
            "mysql://root:hunter2x@localhost:3306/x",
            "mysql://root:hunter2x@127.0.0.1/x",
            "redis://u:hunter2x@[::1]:6379",
            "redis://u:hunter2x@cache.test",
            "mongodb://u:hunter2x@db.invalid",
            "postgres://app:${DB_PASSWORD}@db.acme.io/x",
            "postgres://app:$DB_PASSWORD@db.acme.io/x",
            "postgres://app:{{ db_password }}@db.acme.io/x",
            "postgres://app:<password>@db.acme.io/x",
            "postgres://app:%PGPASS%@db.acme.io/x",
            "postgres://user:password@db.acme.io/x",
            "postgres://user:changeme@db.acme.io/x",
            "postgres://postgres:postgres@db.acme.io/x",
            "postgres://u:xxxx@db.acme.io/x",
            "postgres://u:****@db.acme.io/x",
            "postgres://u:abc@db.acme.io/x",
            "postgres://app@db.acme.io/x",
            "https://db.acme.io/x?u=a:b@c",
            "see a:b@c and user@example.com",
        ];
        for s in must_pass {
            assert!(names(s).is_empty(), "{s} -> {:?}", names(s));
        }
    }

    #[test]
    fn jwt_hits_but_not_the_jwt_io_sample() {
        let part = |n: usize| "a".repeat(n);
        let jwt =
            format!("{}{}.{}{}.{}", "ey", "J".to_string() + &part(20), "ey", "J".to_string() + &part(30), part(43));
        assert_eq!(names(&format!("Authorization: Bearer {jwt}")), ["JWT"]);
        let sample = format!(
            "{}{}.{}{}.{JWT_SAMPLE_SIGNATURE}",
            "ey",
            "J".to_string() + &part(20),
            "ey",
            "J".to_string() + &part(30)
        );
        assert!(names(&sample).is_empty());
        assert!(names("eyJhbGciOi only one part").is_empty());
    }

    #[test]
    fn redact_replaces_each_hit() {
        let pw = format!("{}{}", "s3cr", "Et-77x");
        let t = format!("a postgres://app:{pw}@db.acme.io/x b {}_{} c", "ghp", "a".repeat(36));
        let (r, n) = redact(&t);
        assert_eq!(n, 2);
        assert_eq!(r, format!("a {MARKER}{CONNECTION}]/x b {MARKER}GitHub token] c"));
        assert_eq!(redact("nothing here"), ("nothing here".to_string(), 0));
    }
}
