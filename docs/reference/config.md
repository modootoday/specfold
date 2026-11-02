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
| `[types.<name>]` values (S8) | `fields = { approvers = "people", resolution = "url", review_by = "date", decision = "link" }` adds the keys and their kinds (a list still replaces the built-in keys); `placeholders = [...]` (default `TBD`, `Pending`, `TODO`, `?`); `distinct = [["authors", "approvers"]]`. Checked only where `require_when` requires the field |
| `[forbid]` | retired layouts and identifiers (rule Z1) |
| `[aliases]` | the frozen alias table written by `spec import` |
| `[files]` | `inline_max_bytes`, `scan_max_bytes` (repository text attachments up to this size are read by rule U2; 1 MiB by default), and `[files.store]`: `kind = "dir"` with `path`, or `kind = "command"` with `put`/`get`/`exists` argv templates (see the example) |
| `[[assets]]` | agent skills, rules, subagents and instruction files |

## Checks

| table | controls |
| --- | --- |
| `[check]` | baseline, strict paths, rule severity, `extra_keys`, `count_paths`, `link_ignore`, `d1` (`"all"`, the default, or `"specific"`: rule D1 asks only the most specific document governing a changed file) |
| `[[guards]]` | named checks over files and commit messages (rule U1) |
| `[[plugins]]` | external programs speaking `specfold.plugin.v1` |
| `[translations]` | translations paired by place: `layout`, `languages`, `source_lang`, `strict` (rule R2) |
| `[audit]` | thresholds for stale, stalled and draft documents; `coverage_types` lists types whose `{#ANCHOR}` headings must each be cited (`spec audit --kind coverage`) |
| `[verify]` | deadline, parallel jobs and shell for `spec verify` |
| `[cites]` | files outside documents whose citations rule L6 checks |
| `[write]` | `max_files`: mv, fix, sync and import need `--yes` to write more files than this at once (default 50; 0 = no bound) |
| `[mcp]` | `redact`: hide secrets in what `spec mcp` returns (on by default) |

## Cache

Scan, file-hash and guard results are cached per repository path outside the repository: `$SPECFOLD_CACHE`, else `$XDG_CACHE_HOME/specfold`, `%LOCALAPPDATA%\specfold`, `~/Library/Caches/specfold` or `~/.cache/specfold`. Losing it only costs one slower run. The directory stays bounded on its own: at most once an hour a run removes files unused for 30 days and then the least recently used ones while it holds more than 64 MB; deleting it by hand is always safe.

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
link_ignore = ["site/api/**"]   # rule L8 skips links to these paths
d1 = "specific"                 # D1 skips an overview when a narrower document governs the file

[translations]
layout = "suffix"   # guide:shop/setup-ko translates guide:shop/setup
languages = ["ko", "ja"]

[remotes.org]
path = "../shared-decisions"   # [[org:adr:root/20261030000000]] names a document there

[files]
inline_max_bytes = 1048576     # larger attachments go to the store

[files.store]
kind = "dir"                   # or "command" with put/get/exists, e.g.
path = "~/specfold-store"      # put = ["rclone", "copyto", "{src}", "remote:bucket/{key}"]
```
