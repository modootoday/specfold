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

`spec check` exits 1 on errors, so any CI system can gate on it. `--summary` gives findings per
rule, and `--json` gives one object for tooling.

## Adopt gradually

`spec check --write-baseline` records today's findings in a committed file, so only new findings
fail. `[check] severity` sets a rule to `error`, `warn` or `off`. `[check] strict` lists paths that
never get a baseline.

## Verify sources of truth

`spec verify` runs the `verify` commands of `sot` documents (rule **X1**). `[verify]` sets the
deadline, parallel jobs and the shell.
