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

A frontmatter key outside the common set and the type's fields is a warning (S1). Keep a key for
one type with `[types.<type>] fields = [...]`, or for every type with
`[check] extra_keys = ["team", "review_*"]` (a trailing `*` allows a prefix).

A number in prose can name where it comes from, and `spec check` warns when it drifts (N1);
`spec fix --write` rewrites it. Sources are spec's own counts, never shell commands:

```md
We keep <!-- spec:count find type=adr status=active -->12<!-- /spec:count --> active decisions.
<!-- spec:count rows="Open items" -->3<!-- /spec:count --> rows are open.
```

`find` takes `type`, `owner`, `status` and `tag` (comma for several); `rows=<section>` counts the
table body rows in that section of the same file. A marker opens and closes on one line, does
not nest, and is ignored inside code. Markers in files outside documents are checked only when
listed, e.g. `[check] count_paths = ["README.md", "docs/**/*.md"]`.

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

## In CI

`spec check --format` prints a document CI systems read instead of the text: `github` (workflow annotations, plus a table in the step summary), `sarif` (code scanning), `gitlab` (Code Quality) or `rdjson` (reviewdog). The exit code is the same as without it.

The repository's `action.yml` installs a release, checks the archive against its `.sha256` file, and runs `spec check`. Pin `version` to a tag from the releases page:

```yaml
name: specs
on: [pull_request]
permissions:
  contents: read
  security-events: write   # only for the SARIF upload
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with: { fetch-depth: 0 }
      - uses: modootoday/specfold@<tag>
        with:
          version: <tag>
          format: sarif
          args: --changed origin/${{ github.base_ref }}
      - uses: github/codeql-action/upload-sarif@v3
        if: always()
        with: { sarif_file: spec-check.sarif.json }
```

With `format: github` (the default) the findings show up on the pull request without the upload step. For pre-commit, the repository ships `.pre-commit-hooks.yaml` (`spec-check`, `spec-check-push`); they need pre-commit 4.4.0 or later and `spec` on PATH.
