//! Starter import maps for layouts other ADR tools write. They are printed for the project to
//! edit, never applied directly: custom templates and statuses vary.

pub const NAMES: &[&str] = &["adr-tools", "madr", "log4brains"];

pub fn get(name: &str) -> Option<&'static str> {
    match name {
        "adr-tools" => Some(ADR_TOOLS),
        "madr" => Some(MADR),
        "log4brains" => Some(LOG4BRAINS),
        _ => None,
    }
}

const ADR_TOOLS: &str = r##"# Starter import map for adr-tools (doc/adr/NNNN-title.md; see .adr-dir if moved).
# Edit it, then: spec import --map import.toml
stem_aliases = true             # links name old files: 0001-record-architecture-decisions.md
strip_title_number = true       # "# 1. Record architecture decisions" -> "Record architecture decisions"

[status]
accepted = "active"
proposed = "draft"
rejected = "rejected"
deprecated = "superseded"
"superceded by" = "superseded"  # adr-tools' spelling

[body]
status = "section:Status"
date = "line:Date"

[body.links]
Supercedes = "supersedes"
"Superceded by" = "-"           # the newer record lists it

[[sources]]
glob = "doc/adr/*.md"
exclude = ["doc/adr/README.md", "doc/adr/index.md"]
type = "adr"
key = "created"
name = '^(?P<alias>\d{4})-(?P<slug>.+)\.md$'
"##;

const MADR: &str = r##"# Starter import map for MADR (2.x: docs/adr with "* Status:" bullets; 3.x and 4.x:
# docs/decisions with frontmatter). Edit it, then: spec import --map import.toml
stem_aliases = true
strip_title_number = true

[status]
accepted = "active"
proposed = "draft"
rejected = "rejected"
deprecated = "superseded"
superseded = "superseded"

[[status_patterns]]             # "superseded by ADR-0005" / "superseded by [ADR-0005](0005-x.md)"
pattern = '^superseded by\b(?P<by>.*)$'
status = "superseded"

[body]                          # MADR 2 only; frontmatter wins where it exists
status = "bullet:Status"
date = "bullet:Date"

[[sources]]
glob = "docs/decisions/*.md"
exclude = ["docs/decisions/README.md", "docs/decisions/index.md", "docs/decisions/adr-template*.md"]
type = "adr"
key = "created"
name = '^(?P<alias>\d{4})-(?P<slug>.+)\.md$'

[[sources]]
glob = "docs/adr/*.md"
exclude = ["docs/adr/README.md", "docs/adr/index.md", "docs/adr/template.md"]
type = "adr"
key = "created"
name = '^(?P<alias>\d{4})-(?P<slug>.+)\.md$'
"##;

const LOG4BRAINS: &str = r##"# Starter import map for Log4brains (docs/adr/YYYYMMDD-title.md, and
# packages/<name>/docs/adr for package records). Edit it, then: spec import --map import.toml
stem_aliases = true

[status]
accepted = "active"
proposed = "draft"
draft = "draft"
rejected = "rejected"
deprecated = "superseded"
"superseded by" = "superseded"

[body]
status = "bullet:Status"
date = "bullet:Date"

[[sources]]
glob = "docs/adr/*.md"
exclude = ["docs/adr/README.md", "docs/adr/index.md", "docs/adr/template.md"]
type = "adr"
key = "name"
name = '^(?P<key>\d{8})-(?P<slug>.+)\.md$'

[[sources]]
glob = "packages/*/docs/adr/*.md"
exclude = ["packages/*/docs/adr/README.md", "packages/*/docs/adr/index.md", "packages/*/docs/adr/template.md"]
type = "adr"
key = "name"
name = '^(?P<key>\d{8})-(?P<slug>.+)\.md$'
"##;

#[cfg(test)]
mod tests {
    #[test]
    fn every_preset_is_a_valid_map() {
        for n in super::NAMES {
            let m: crate::import::ImportMap =
                toml::from_str(super::get(n).unwrap()).unwrap_or_else(|e| panic!("{n}: {e}"));
            assert!(!m.sources.is_empty(), "{n}");
        }
    }
}
