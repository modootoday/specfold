//! Reading a manifest change line by line: whether only version values moved, and which packages.

use std::collections::BTreeSet;

/// The manifests and lockfiles whose version-only changes are recognised.
const MANIFESTS: &[&str] = &["package.json", "bun.lock", "Cargo.toml", "Cargo.lock"];

/// The packages whose versions a change of manifest `path` from `old` to `new` moves, when moving
/// versions (and the integrity hashes that follow them) is all it does; `None` otherwise.
pub(super) fn version_bump(path: &str, old: &str, new: &str) -> Option<BTreeSet<String>> {
    if !is_manifest(path) {
        return None;
    }
    let (old, new): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    if old.len() != new.len() {
        return None;
    }
    let mut moved = BTreeSet::new();
    for (i, (a, b)) in old.iter().zip(&new).enumerate() {
        if a == b {
            continue;
        }
        if normalize(a) != normalize(b) {
            return None;
        }
        moved.insert(package_of(&new, i)?);
    }
    (!moved.is_empty()).then_some(moved)
}

/// Whether `path` is a manifest or lockfile this module reads.
pub(super) fn is_manifest(path: &str) -> bool {
    MANIFESTS.contains(&path.rsplit('/').next().unwrap_or(path))
}

/// The line with every version value and integrity hash replaced by a placeholder.
fn normalize(line: &str) -> String {
    if line.trim_start().starts_with("checksum = ") {
        return "checksum".into();
    }
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let prev = if i == 0 { ' ' } else { chars[i - 1] };
        if let Some(end) = hash_end(&chars, i).filter(|_| !prev.is_ascii_alphanumeric()) {
            out.push('#');
            i = end;
        } else if chars[i].is_ascii_digit() && matches!(prev, '"' | '@' | '^' | '~' | '=' | '<' | '>') {
            out.push('0');
            i = version_end(&chars, i);
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Where an integrity hash (`sha512-<base64>`) starting at `i` ends.
fn hash_end(chars: &[char], i: usize) -> Option<usize> {
    let head: String = chars[i..].iter().take(7).collect();
    let prefix = ["sha1-", "sha256-", "sha384-", "sha512-"].into_iter().find(|p| head.starts_with(p))?;
    let start = i + prefix.len();
    let len = chars[start..].iter().take_while(|&&c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')).count();
    (len > 0).then_some(start + len)
}

/// Where a version (`1.2.3`, `7.0.0-dev.20260707.2`, `1.0.0+build`) starting at `i` ends.
fn version_end(chars: &[char], i: usize) -> usize {
    let mut j = i;
    while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
        j += 1;
    }
    if j < chars.len() && matches!(chars[j], '-' | '+') {
        j += 1;
        while j < chars.len() && (chars[j].is_ascii_alphanumeric() || matches!(chars[j], '.' | '-' | '+')) {
            j += 1;
        }
    }
    j
}

/// The package line `i` of a manifest belongs to: its key (`"pkg": "1.0.0"`, `pkg = "1"`), or for
/// a `version` or `checksum` line the nearest `name` above it.
fn package_of(lines: &[&str], i: usize) -> Option<String> {
    let key = key_of(lines[i])?;
    if key != "version" && key != "checksum" {
        return Some(key);
    }
    lines[..i].iter().rev().find(|l| key_of(l).as_deref() == Some("name")).and_then(|l| value_of(l))
}

/// The key of a `"key": value` or `key = value` line.
fn key_of(line: &str) -> Option<String> {
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix('"') {
        return rest.split_once('"').map(|(k, _)| k.to_string());
    }
    let (k, _) = t.split_once(" =")?;
    (!k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))).then(|| k.to_string())
}

/// The quoted value of a `"key": "value"` or `key = "value"` line.
fn value_of(line: &str) -> Option<String> {
    let t = line.trim_start();
    let rest = match t.strip_prefix('"') {
        Some(r) => r.split_once('"')?.1,
        None => t.split_once('=')?.1,
    };
    let v = rest.trim_start_matches([':', '=', ' ']).strip_prefix('"')?;
    v.split_once('"').map(|(v, _)| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_only_package_json_names_the_package() {
        let old = "{\n  \"name\": \"app\",\n  \"devDependencies\": {\n    \"@acme/tool\": \"0.8.2\"\n  }\n}\n";
        let new = old.replace("0.8.2", "0.9.0");
        let moved = version_bump("package.json", old, &new).unwrap();
        assert_eq!(moved.into_iter().collect::<Vec<_>>(), ["@acme/tool"]);
    }

    #[test]
    fn lockfile_entry_with_hash_is_version_only() {
        let old = "    \"@acme/tool\": [\"@acme/tool@0.8.2\", \"\", { \"bin\": \"x\" }, \"sha512-AAA+b/c==\"],\n";
        let new = "    \"@acme/tool\": [\"@acme/tool@0.9.0\", \"\", { \"bin\": \"x\" }, \"sha512-ZZZ9=\"],\n";
        assert!(version_bump("bun.lock", old, new).unwrap().contains("@acme/tool"));
    }

    #[test]
    fn cargo_lock_version_and_checksum_name_the_crate() {
        let old = "[[package]]\nname = \"left\"\nversion = \"1.0.0\"\nchecksum = \"abc\"\n";
        let new = "[[package]]\nname = \"left\"\nversion = \"1.0.1\"\nchecksum = \"def\"\n";
        assert_eq!(version_bump("Cargo.lock", old, new).unwrap().into_iter().collect::<Vec<_>>(), ["left"]);
    }

    #[test]
    fn other_changes_are_not_version_only() {
        let old = "{\n  \"scripts\": {\n    \"build\": \"make\"\n  }\n}\n";
        assert_eq!(version_bump("package.json", old, &old.replace("make", "make all")), None);
        assert_eq!(version_bump("package.json", old, &old.replace("}\n}", "}, \"x\": \"1.0.0\"\n}")), None);
        assert_eq!(version_bump("package.json", "  \"port\": 3000\n", "  \"port\": 3001\n"), None);
        assert_eq!(version_bump("src/main.rs", "v = \"1.0\"\n", "v = \"2.0\"\n"), None);
    }
}
