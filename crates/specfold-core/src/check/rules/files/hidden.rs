//! H1: no invisible characters in documents or agent assets. A zero-width space or a
//! bidirectional control hides text from a reviewer while an agent still reads it.

use crate::assets;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};

pub struct HiddenChars;

/// The name of a character that should not appear in Markdown a person reviews.
fn hidden(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{200B}' => "zero-width space",
        '\u{2060}' => "word joiner",
        '\u{FEFF}' => "zero-width no-break space (BOM)",
        '\u{202A}'..='\u{202E}' => "bidirectional embedding or override",
        '\u{2066}'..='\u{2069}' => "bidirectional isolate",
        _ => return None,
    })
}

fn scan(path: &str, text: &str, out: &mut Vec<Finding>) {
    for (i, line) in text.lines().enumerate() {
        for (col, c) in line.chars().enumerate() {
            let Some(name) = hidden(c) else { continue };
            // A byte-order mark on the first line is left alone: removing "the first" one there
            // could take the real one at the start of the file.
            if i == 0 && c == '\u{FEFF}' {
                continue;
            }
            let msg = format!("hidden character U+{:04X} ({name}) at column {}", c as u32, col + 1);
            out.push(Finding::warn("H1", path, i + 1, msg).fix_replace(c.to_string(), ""));
        }
    }
}

impl Rule for HiddenChars {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "H1",
            title: "no invisible characters in documents or agent assets",
            explain: "Agent files are scanned whether declared in spec.toml or not: AGENTS.md, CLAUDE.md, GEMINI.md, .github/copilot-instructions.md, .agents/skills/*/SKILL.md, .claude/skills/*/SKILL.md, .cursor/rules/*.mdc, .claude/agents/*.md and .claude/commands/*.md. Zero-width spaces, word joiners, a byte-order mark after the start of the file, and bidirectional embedding, override or isolate controls do not show in an editor or a diff, but agents and tools read them: a link or a command can look right and not be. Each one is reported per line with its code point; `spec fix --rule H1 --write` removes it. Joiners used inside emoji and scripts (U+200C, U+200D) are not reported. Reported as warnings.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            for p in std::iter::once(&d.path).chain(&d.chapters) {
                if let Some(t) = ctx.current(p) {
                    scan(p, &t, out);
                }
            }
        }
        for (p, t) in agent_files(ctx) {
            scan(&p, &t, out);
        }
    }
}

/// Declared agent assets and the well-known agent files, each once, within scope. Agent files
/// are read whether or not spec.toml declares them, so the usual ones are included too.
pub(super) fn agent_files(ctx: &Ctx) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    let mut out = vec![];
    for a in assets::load(&ctx.corpus.cfg) {
        seen.insert(a.path.clone());
        if ctx.scope.covers_path(&a.path) {
            out.push((a.path, a.text));
        }
    }
    for p in well_known(&ctx.corpus.cfg.root) {
        if seen.insert(p.clone())
            && ctx.scope.covers_path(&p)
            && let Some(t) = ctx.current(&p)
        {
            out.push((p, t.to_string()));
        }
    }
    out
}

/// Agent instruction and skill files at their usual places, relative to the root.
fn well_known(root: &std::path::Path) -> Vec<String> {
    let mut out: Vec<String> = ["AGENTS.md", "CLAUDE.md", "GEMINI.md", ".github/copilot-instructions.md"]
        .iter()
        .filter(|p| root.join(p).is_file())
        .map(|p| p.to_string())
        .collect();
    let mut listed = |dir: &str, each: &dyn Fn(&std::path::Path) -> Option<String>| {
        let Ok(rd) = std::fs::read_dir(root.join(dir)) else { return };
        let mut found: Vec<String> = rd.flatten().filter_map(|e| each(&e.path())).collect();
        found.sort();
        out.extend(found);
    };
    for skills in [".agents/skills", ".claude/skills"] {
        listed(skills, &|p| {
            let name = p.file_name()?.to_string_lossy().to_string();
            p.join("SKILL.md").is_file().then(|| format!("{skills}/{name}/SKILL.md"))
        });
    }
    for (dir, ext) in [(".cursor/rules", "mdc"), (".claude/agents", "md"), (".claude/commands", "md")] {
        listed(dir, &|p| {
            let name = p.file_name()?.to_string_lossy().to_string();
            (p.is_file() && p.extension()? == ext).then(|| format!("{dir}/{name}"))
        });
    }
    out
}
