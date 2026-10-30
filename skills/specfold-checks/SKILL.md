---
name: specfold-checks
description: Run and act on specfold's checks - spec check (document rules), spec verify (the verify commands sources of truth declare), spec guard (denied terms, patterns and files) and spec audit - diagnose setup failures with spec doctor, read each finding with spec explain, apply mechanical fixes with spec fix, and manage the [check] baseline and guard baselines so only new findings fail. Use when spec check, spec verify or spec guard fails in a hook or by hand, when adopting specfold on a repository that already has broken references, or when asked to audit documents. Not for creating documents (specfold-documents).
metadata:
  tier: open
  level: L2
  domain: documentation
  install: optional
  keywords: "specfold, spec check, spec verify, spec guard, spec audit, baseline, broken links, pre-commit, pre-push"
  requires-bin: "spec"
---

# Acting on specfold checks

Exit codes: 0 nothing to report, 1 findings, 2 usage error, 3 environment (a missing tool, an
unreadable file). Each finding names its rule, its file and line, and a fix. When a command
fails before any rule runs (spec.toml does not load, exit 3), run `spec doctor` first: it lists
setup problems with their fixes and writes nothing.

## Scope the run

| Situation        | Command                                   |
| ---------------- | ----------------------------------------- |
| Before a commit  | `spec check --staged`                     |
| Before a push    | `spec check --changed origin/main`        |
| Rules in code    | `spec verify --changed origin/main`       |
| Whole repository | `spec check` (add `--guards` for guards)  |
| One rule         | `spec check --rule L1 --rule L2 --json`   |

Staged and ranged runs also run the guards; a full run runs them only with `--guards`.

## Read, then fix

```sh
spec explain L2                # why the rule exists and how to fix it
spec fix --rule L2             # preview
spec fix --rule L2 --write
```

`spec fix` only applies changes that need no judgement. For an ambiguous reference it lists
the candidates; choose one with `--pick <id>` on that `path:line`. Everything else is fixed by
hand, or by changing the document's status or relations with `spec set`.

Some findings carry a command to finish by hand instead of a replacement: a missing required
section (S7) names `spec edit <id> --new-section <name> --text "…" --write`; fill in the text.

Rules for agent files: H1 (hidden characters; `spec fix --rule H1 --write` removes them), A1
(skill and agent format), A2 (two assets with one name), A3 (paths and `spec` commands that do
not exist) and A4 (a path under one person's home directory; write it relative to the
repository; the CI runner's /home/runner/ is not reported). A3 does not read git refs such as
`origin/main` as paths. H1 and A4 also cover AGENTS.md, CLAUDE.md and the usual skill
directories when spec.toml declares no assets.

R1 (translation behind its source, or no `source_rev`) is fixed by updating the translation's
text to match the source, then `spec set <id> --source-rev`. The warning names the changed
sections (`in Install (1 paragraph changed), Support (new)`) when git has the recorded
revision: update those sections. Never run `--source-rev` without updating the text: it only
records that the translation matches. `spec census --translations` lists every translation's
state and exits 1 when one is behind or a language lacks a source; `--fail-on stale` ignores
missing translations and `--fail-on none` only reports.

R2 (a translation out of place) means the `[translations]` layout and the frontmatter disagree,
or a document named like a translation has no source. Rename the document into place with
`spec mv`, correct `lang` or `rel.translation_of`, or add the source; do not loosen the layout.

R1 errors when a translation names itself or another translation as its source: point
`rel.translation_of` at the original the message names. Bad `lang` or `source_rev` values and two
translations in one language are warnings.

A `spec verify` failure means code no longer keeps a rule a source of truth states. Decide
which is right: fix the code, or update the source of truth in the same change. Never edit the
verify command just to make it pass.

## In editors and CI

`spec check --format editor` prints `path:line:col: severity RULE: message` for problem
matchers; `spec lsp` shows the same findings live, with quick fixes. CI annotations come from
`--format github`, `sarif`, `gitlab` or `rdjson`.

## Baselines

A repository that adopts specfold with findings it cannot fix at once records them, so only
new ones fail:

```sh
spec check --write-baseline        # full run on a clean tree; writes [check] baseline
spec guard --write-baseline        # each guard's current hits
```

Commit the baseline files. Fixing a finding later shrinks the baseline; never add to it to get
a change through. `--no-baseline` shows everything, and `[check] severity` sets a rule to
`error`, `warn` or `off` for the whole repository.

## Guards

`spec guard` checks files and commit messages in a range for denied terms, patterns, files,
sizes and secret formats. A hit is masked in the output. Fix the content; an exception goes in
the guard's `allow` list in `spec.toml` with a reason, reviewed like code.

## Audits

```sh
spec audit --kind stale --kind gap
spec audit --pack                  # review packs for contradictions and stale statements
spec audit record --from review.md --title "Quarterly review" --write
```

Audits find what passes the rules yet needs attention: stale documents, orphans, owners
without a source of truth, stalled plans and open items.

## Report

The command and scope that ran, the finding counts by rule before and after, what was fixed
mechanically and what by hand, and any baseline change with its reason.
