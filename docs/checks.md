---
title: Checks, guards, plugins and audits
description: Rules, guards, plugins and audits.
nav_order: 3
---

# Checks, guards, plugins and audits

## Checks

`spec check` runs the rules; `spec explain <rule>` says why each exists and how to fix it.

```sh
spec check --staged     # before committing
spec check --new        # documents not in the last commit
spec check --summary    # findings per rule; -q lists errors only
```

## Guards

Guards (`[[guards]]`, `spec guard`) check any file and commit message in a range for denied
terms, patterns, files, sizes and secret formats. Terms may come from a file outside the
repository or an environment variable, and are masked in output.

## Plugins

Plugins (`[[plugins]]`) are programs in any language that speak `specfold.plugin.v1` JSON over
stdin and stdout to add rules, importers, exporters, audits and event handlers. Only programs
named in `spec.toml` run.

## Audits

`spec audit` finds what passes the rules yet needs attention: stale documents, orphans, owners
without a source of truth, stalled plans, finished plans (every task ticked but still active, or
archived with tasks open), open items, and agent assets that drifted. `--pack` groups documents
for a reviewer, and `spec audit record` stores a review as a sealed audit document.
