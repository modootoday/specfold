---
name: specfold-import
description: Move an existing documentation layout (docs/adr, plans, RFCs, design notes, raw research output, hand-kept indexes, "moved to" stubs) into specfold with spec import and an import map - one rule per source pattern with its type, owner, key and status mapping, wrap for raw files, drop for old indexes, redirect for stubs - then review the plan, apply it, fix what is mechanical and baseline the rest. Use when a repository already has design or planning documents and is adopting specfold, or when asked to migrate docs into .spec. Not for a repository with no documents yet (specfold-setup).
metadata:
  tier: open
  level: L3
  domain: documentation
  install: optional
  keywords: "specfold, spec import, import map, migration, ADR migration, docs migration, aliases, redirect stubs"
  requires-bin: "spec"
---

# Importing an existing layout

Set up `spec.toml` first (specfold-setup), so owners and types exist. The import never deletes
history: old paths and ids are written to an alias table, so citations to them keep resolving
until they are rewritten.

## 1. Inventory

```sh
git ls-files | grep -iE '(^|/)(docs?|adr|rfcs?|plans?|design|research|notes)/' | sed 's|/[^/]*$||' | sort | uniq -c | sort -rn | head -40
```

For each directory decide: which type its files are, which owner keeps them, how the key is
chosen, and which statuses map to which.

## 2. Write the map

```toml
stem_aliases = true            # old documents referred to each other by bare name

[status]                       # old status -> new, for every source
accepted = "active"
deprecated = "superseded"

[keys]                         # old frontmatter key -> new; "-" drops; rel.<kind> moves into rel
supersedes = "rel.supersedes"
applies_to = "rel.governs"
owner = "-"

[[sources]]                    # one decision per file, dated in its name
glob = "docs/adr/*.md"
type = "adr"
key = "name"
name = '^(?P<key>\d{8})-(?P<slug>.+)\.md$'
default_status = "active"

[[sources]]                    # "> Moved to `path`" stubs become alias rows
glob = "docs/**/*.md"
redirect = '^> Moved to `(?P<target>[^`]+)`'

[[sources]]                    # raw output without Markdown becomes an evidence document
glob = "research/*/"
type = "evidence"
wrap = true

[[sources]]                    # hand-kept indexes the graph now derives
glob = "{docs/INDEX.md,docs/README.md}"
drop = true
```

Rules match in order and a file is taken by the first rule that matches it, so put narrow
rules before broad ones. A glob ending in `/` takes directories as bundles: the Markdown file
is the main document, other Markdown files are chapters, the rest are attachments.
`sibling_dir = true` adds the directory named like a file to that file's bundle. `key` is
`name` (from the `key` group), `first-commit` or `created`. `owner` is `auto` (from governs,
then from code paths the text mentions) or a fixed owner id.

Layouts that keep metadata in the body (adr-tools, MADR 2) say where with `[body]`; frontmatter
wins when both have a value, and a file where the value is missing gets an `unresolved` entry:

```toml
[body]
status = "section:Status"      # first line under "## Status"; or bullet:Status, line:Status
date = "line:Date"             # "Date: 2016-02-12"; MADR 2 uses bullet:Date
[body.links]                   # a line opening with the phrase, then [title](file) links
Supercedes = "supersedes"      # adr-tools' spelling
"Superceded by" = "-"          # the other side; the newer record already says it
```

Status values match the `[status]` table as written or in lower case. Link targets are old file
names, so set `stem_aliases = true` for them to resolve after the apply.

## 3. Review the plan

```sh
spec import --map import.toml --plan --json > plan.json
```

Read every row with an `unresolved` entry: a status with no mapping, frontmatter that does not
parse, a stub whose target is not imported. Fix the map or the source and plan again until
nothing is unresolved that you did not mean.

## 4. Apply on a clean tree

```sh
spec import --map import.toml --apply --write
spec fix --rule L1 --rule L2 --write
spec check
```

Commit the import as one change of its own, separate from any content edits, so the move is
reviewable. Findings that were already broken before the import (citations to files that never
existed) go into the baseline (specfold-checks), and the baseline becomes the cleanup list.

To import owner by owner, list the owners not yet imported in `spec.toml`:

```toml
[owners]
pending = ["services/legacy"]
```

Documents already in type directories there are read as usual; files of the old layout raise no
findings, so new files there need no baseline entries. Take an owner off the list in the change
that imports it.

## Report

The map's rules, the plan's row counts by kind, the unresolved rows and how each was settled,
the `spec check` counts by rule after the apply, and what went into the baseline.
