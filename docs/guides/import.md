---
title: Importing an existing layout
description: Bring documents in from another layout through an import map.
redirect_from: [/import/]
---

# Importing an existing layout

> [!TIP]
> Describe the old layout in an import map, review the plan, then apply it. Old ids and paths
> keep resolving through a frozen alias table.

## Steps

1. **Start from a preset or write a map.** `spec import --preset adr-tools|madr|log4brains`
   prints a starter map for those layouts; edit it for your templates and statuses. People
   fields survive into `adr` front matter as lists: `decision-makers`, `deciders` and
   `* Deciders:` body lines become `decision_makers`, and `consulted`/`informed` keep their names.
2. **Set the alias table.** Applying needs `[aliases] file` in `spec.toml`; old file names,
   numbers and links resolve through it.
3. **Review the plan.** `spec import --map import.toml` changes nothing. Settle every line
   marked `!` (the plan's `next` names the `[status]` line to add), then plan again.
4. **Apply.** `--apply` alone previews; `--apply --write` moves everything and writes the alias
   table.

```console
$ spec import --preset madr > import.toml
$ printf '\n[aliases]\nfile = ".spec/_aliases.tsv"\n' >> spec.toml
$ spec import --map import.toml
$ spec import --map import.toml --apply --write
$ spec check
```

## The map

| Key | What it does |
| --- | ------------ |
| `[[sources]]` | `glob`, `exclude`, `type`, `owner`, `key` (`name`, `created`, `first-commit`) and a `name` regex with `key`, `slug` and `alias` groups |
| `[status]` | Old status to new status, matched as written, then in lower case |
| `[[status_patterns]]` | A regex and a status, for values with more than a word |
| `[body]` | Where the body keeps the status and the date |
| `[body.links]` | A line phrase and the relation it records |
| `[keys]` | Old frontmatter key to new key, `rel.<kind>`, or `-` to drop it |
| `stem_aliases` | Also record each old file name as an alias |
| `strip_title_number` | `# 2. Use X` becomes the title `Use X` |

### Status patterns

```toml
[[status_patterns]]
pattern = '^superseded by\b(?P<by>.*)$'
status = "superseded"
```

The `by` group names the replacing record, such as `[ADR-0002](0002-use-y.md)` or `ADR-0002`.
When that record is in the same import, it gets `rel.supersedes` to the old one. When it is
not, the row needs a decision.

### Body metadata

```toml
[body]
status = "section:Status"     # first line under "## Status"
date = "line:Date"            # "Date: 2024-03-01"
drop_status_section = false   # true removes the Status section once it is in frontmatter

[body.links]
Supersedes = "supersedes"
"Superseded by" = "-"         # the newer record lists the link
```

`status` and `date` take `section:<heading>`, `bullet:<label>` (`* Label: value`) or
`line:<label>`. Frontmatter wins where both have a value. The date becomes `created`.

### What the plan reports

- Relative links between imported records are rewritten to the new paths. Links to files
  outside the import are left for rules **L1** and **L2**.
- A body that still has its Status section is noted, not changed, unless
  `drop_status_section = true`.
- A file name that is mostly non-ASCII gives a slug without its words. The title is kept, and
  `spec mv <id> --slug <slug> --write` renames it.

## After the move

Rules **I2** and **I3** keep the alias table honest: aliases point at one document, and the
table gains no rows. `[forbid]` keeps retired layouts from coming back (rule **Z1**).
