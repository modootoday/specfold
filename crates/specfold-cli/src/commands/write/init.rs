//! `spec init`: a starter spec.toml.

use crate::cli::args::{global::Global, write::InitArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};
use specfold_core::config::starter::{self, StarterKind};
use specfold_core::config::{defaults, upgrade};

fn parse_kind(s: &str) -> Result<StarterKind> {
    let (body, manifest) = match s.strip_suffix(":manifest") {
        Some(b) => (b, true),
        None => (s, false),
    };
    let Some((pattern, id)) = body.split_once('=') else {
        bail!("--owner-kind '{s}' is not pattern=id-template, e.g. 'services/*={{1}}'")
    };
    let (pattern, id) = (pattern.trim(), id.trim());
    // C140: what is written here becomes every owner id, so check it before it lands.
    if pattern.is_empty() || id.is_empty() {
        bail!("--owner-kind '{s}' needs a folder pattern and an id template, e.g. 'services/*={{1}}'");
    }
    if body.contains(':') {
        bail!("--owner-kind '{s}': the only suffix is ':manifest', e.g. 'libs/*=lib.{{1}}:manifest'");
    }
    if let Some(why) = specfold_core::workspace::owners::patterns::kind_pattern_problem(pattern) {
        bail!("--owner-kind '{s}': {why}");
    }
    let stars = specfold_core::workspace::owners::patterns::kind_stars(pattern);
    let mut sample = String::new();
    let mut rest = id;
    while let Some(i) = rest.find('{') {
        sample.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('}') else { bail!("--owner-kind '{s}': unclosed '{{' in the id template") };
        let n: usize = rest[i + 1..i + j].parse().unwrap_or(0);
        if n == 0 || n > stars {
            bail!(
                "--owner-kind '{s}': {} names no '*' in the pattern (it has {stars}); use {{1}}..{{{stars}}}",
                &rest[i..=i + j]
            );
        }
        sample.push('x');
        rest = &rest[i + j + 1..];
    }
    sample.push_str(rest);
    if !owner_like(&sample) {
        bail!("--owner-kind '{s}': the id template makes ids like '{sample}'; use a-z, 0-9, '.', '-', '_'");
    }
    Ok(StarterKind { pattern: pattern.into(), id: id.into(), manifest })
}

fn owner_like(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_'))
        && s.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
}

/// `spec init --upgrade`: reads the file as text, since an old file may not parse.
fn upgrade(g: &Global, a: &InitArgs) -> Result<Output> {
    let mut dir = match &g.dir {
        Some(d) => d.canonicalize()?,
        None => std::env::current_dir()?,
    };
    let path = loop {
        let p = dir.join("spec.toml");
        if p.is_file() {
            break p;
        }
        if !dir.pop() {
            bail!("no spec.toml here or above; create one: spec init --write");
        }
    };
    let raw = std::fs::read_to_string(&path)?;
    // C191: a leading BOM hides the first table from the line scan; it is put back on write.
    let bom = if raw.starts_with('\u{feff}') { "\u{feff}" } else { "" };
    let text = raw.strip_prefix('\u{feff}').unwrap_or(&raw).to_string();
    // C170: a file from a newer version is refused as `spec check` refuses it.
    if let Some(n) = upgrade::schema_of(&text).filter(|n| *n > defaults::schema()) {
        return Err(specfold_core::config::reader::newer_schema(&path, n));
    }
    let (new, changes) = upgrade::plan(&text, upgrade::RENAMED);
    let mut o =
        Output::new("init").unpaged().field("path", path.display().to_string()).field("changes", changes.clone());
    if changes.is_empty() {
        return Ok(o
            .head(format!("{} is current (schema {})", path.display(), defaults::schema()))
            .next("spec doctor"));
    }
    for c in &changes {
        o = o.head(c.clone());
    }
    if a.write {
        std::fs::write(&path, format!("{bom}{new}"))?;
        Ok(o.field("written", true).foot("written").next("spec doctor"))
    } else {
        Ok(o.field("written", false).next("spec init --upgrade --write"))
    }
}

pub fn run(g: &Global, a: &InitArgs) -> Result<Output> {
    if a.upgrade {
        return upgrade(g, a);
    }
    let cfg = session::config(g)?;
    if !cfg.implicit {
        bail!("{} already has a spec.toml", cfg.root.display());
    }
    if !owner_like(&a.root) {
        bail!("--root '{}' is not an owner id; use a-z, 0-9, '.', '-', '_' (e.g. --root ws)", a.root);
    }
    let kinds = a.owners.iter().map(|k| parse_kind(k)).collect::<Result<Vec<_>>>()?;
    let text = starter::render(&a.root, &kinds);
    let path = cfg.root.join("spec.toml");
    let mut o = Output::new("init")
        .unpaged()
        .field("path", path.display().to_string())
        .field("content", text.clone())
        .head(format!("spec.toml for {}", cfg.root.display()))
        .foot(text);
    if a.write {
        std::fs::write(&path, o.summary["content"].as_str().unwrap_or_default())?;
        o = o.foot("written").next("spec new sot --governs <path> --title '…'");
    } else {
        o = o.next("spec init --write");
    }
    Ok(o)
}
