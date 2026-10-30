---
title: Configuration
description: The spec.toml tables and what each one controls.
---

# Configuration

> [!TIP]
> `spec init` writes a starter `spec.toml`. `spec explain config` prints this list from the CLI
> itself.

## Layout and owners

| table | controls |
| --- | --- |
| `[spec]` | format version and the document directory name (`.spec`) |
| `[owners]` | the root owner, owner patterns, manifests and retired directories |
| `[public]` | published paths that hold no documents, and where their documents go |
| `[local]` | the git-ignored directory for drafts and private notes |
| `[external.<name>]` | directories outside the repository whose citations are checked |
| `[remotes.<name>]` | other repositories on disk whose documents are named `<name>:<id>`, read-only |

## Documents

| table | controls |
| --- | --- |
| `[types.<name>]` | override, disable or add a type: key, statuses, fields, transitions, required sections |
| `[forbid]` | retired layouts and identifiers (rule Z1) |
| `[aliases]` | the frozen alias table written by `spec import` |
| `[files]` | the attachment size limit and the store for larger files |
| `[[assets]]` | agent skills, rules, subagents and instruction files |

## Checks

| table | controls |
| --- | --- |
| `[check]` | baseline, strict paths, rule severity, `extra_keys`, `count_paths` |
| `[[guards]]` | named checks over files and commit messages (rule U1) |
| `[[plugins]]` | external programs speaking `specfold.plugin.v1` |
| `[translations]` | translations paired by place: `layout`, `languages`, `source_lang`, `strict` (rule R2) |
| `[audit]` | thresholds for stale, stalled and draft documents |
| `[verify]` | deadline, parallel jobs and shell for `spec verify` |
| `[cites]` | files outside documents whose citations rule L6 checks |
| `[mcp]` | `redact`: hide secrets in what `spec mcp` returns (on by default) |

## Example

```toml
[owners]
root = "root"

[[owners.kinds]]
pattern = "services/*"
id = "{1}"
manifest = false

[check]
extra_keys = ["team", "review_*"]
count_paths = ["README.md", "docs/**/*.md"]

[translations]
layout = "suffix"   # guide:shop/setup-ko translates guide:shop/setup
languages = ["ko", "ja"]

[remotes.org]
path = "../shared-decisions"   # [[org:adr:root/20261030000000]] names a document there
```
