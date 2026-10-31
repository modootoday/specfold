---
title: Checks in CI and hooks
description: Run spec check before each commit and in continuous integration.
---

# Checks in CI and hooks

> [!TIP]
> Run `spec check --staged` in a pre-commit hook and `spec check` in CI. specfold prints hook
> snippets but never installs them itself.

## Steps

1. **Print a hook.** `spec hook print --for git` (or `pre-commit`, `lefthook`, `husky`).
2. **Add it yourself** to the hook manager you already use.
3. **Run the full check in CI**, with guards over the pushed range.

## Commands

```console
$ spec hook print --for lefthook
$ spec check --staged -q
$ spec check
$ spec guard --range origin/main..HEAD
```

`spec guard --range A..B` reads the files as they are at `B` (HEAD when `B` is left out), not
as they are checked out, and also reads every version a commit inside the range wrote: a value
one commit adds and a later one removes is still pushed, so it still fails the guard.

`spec check` exits 1 on errors, so any CI system can gate on it. `--summary` gives findings per
rule, and `--json` gives one object for tooling.

## Adopt gradually

`spec check --write-baseline --yes` records today's findings in a committed file, so only new findings
fail. `[check] severity` sets a rule to `error`, `warn` or `off`. `[check] strict` lists paths that
never get a baseline.

## Verify sources of truth

`spec verify` runs the `verify` commands of `sot` documents (rule **X1**). `[verify]` sets the
deadline, parallel jobs and the shell.

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
      - uses: modootoday/specfold@<tag>
        with:
          format: sarif
      - uses: github/codeql-action/upload-sarif@v3
        if: always()
        with: { sarif_file: spec-check.sarif.json }
```

On a pull request the action checks only what it changes, from the merge base with the base branch (`changed: auto`, deepening a shallow checkout as needed); `changed: off` checks everything. The release installed is the tag the action is pinned to; pinned to a branch, a commit SHA or used as `./`, the action has no release to match and needs `version: <tag>` (GitHub fills in a composite action's own ref only for release-style pins). `fail-on` limits which rules fail the step, and `fail-on: none` only reports. With `format: github` (the default) the findings show up on the pull request without the upload step. For pre-commit, the repository ships `.pre-commit-hooks.yaml` (`spec-check`, `spec-check-push`); they need pre-commit 4.4.0 or later and `spec` on PATH.
