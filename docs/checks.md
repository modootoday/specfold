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

A document is stale when its governed code changed more than `audit.stale_days` after `updated`.
To track it by commit instead, pin it with `spec set <id> verified_at=HEAD` after checking the body
against the code: `spec audit --kind stale` then reports any later commit to the governed paths,
or a pin that is no longer a commit, and suggests the `git diff` to read before re-pinning.
