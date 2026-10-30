---
title: Rules
description: Every rule spec check runs, grouped by family.
---

# Rules

> [!TIP]
> Each finding names a rule id. `spec explain <id>` says why the rule exists and how to fix a
> finding; `spec check --rule <id>` runs just that rule.

## Structure

| rule | checks that |
| --- | --- |
| **S1** | frontmatter parses and has the required keys |
| **S2** | id matches the path |
| **S3** | every file in a spec directory belongs to a document |
| **S4** | status is one the type allows |
| **S5** | dates are ISO and updated is not before created |
| **S6** | a status has the fields it requires |
| **S7** | a document has the sections its type requires (`spec new` writes them; the finding names the `spec edit --new-section` command) |
| **S8** | a required field has a usable value: not a placeholder (`TBD`, `Pending`, `TODO`, `?` by default), of its declared kind, distinct where asked (warning) |

## Identity

| rule | checks that |
| --- | --- |
| **I1** | ids are unique across every root |
| **I2** | aliases do not shadow ids and point at one document |
| **I3** | the alias table gains no rows |

## Links

| rule | checks that |
| --- | --- |
| **L1** | every [[reference]] resolves to one document |
| **L2** | documents are cited by id, not by path |
| **L3** | relations resolve and point at the right types |
| **L4** | anchors are unique within a document |
| **L5** | governed paths exist |
| **L6** | citations outside documents still resolve |
| **L7** | pinned remote documents are unchanged |
| **L8** | local Markdown links, images and `#fragments` lead somewhere (warn) |
| **L9** | images have alt text (warn) |

A reference or relation written `<name>:<id>` names a document in another repository listed
under `[remotes.<name>]`. L1 and L3 check it when that directory is on disk, with the same type
rules as local targets, and skip it when the directory is absent, so a CI job without that
checkout does not fail. `spec show <name>:<id>` reads it; nothing writes there.
L8 checks plain links offline: a relative path must exist and a `#fragment` must match a
heading's GitHub slug (a repeat gets `-1`, `-2`) or a `{#anchor}` in the target file. URLs and
ids are skipped, and so are targets matching `[check] link_ignore` globs.
L6 checks `[[<name>:<id>]]` in files listed under `[cites]` the same way. For checking only, a
translation may name a remote source (`rel.translation_of: [org:guide:root/setup]`); R1 compares
its `source_rev` with the remote body, and `spec set <id> --source-rev` records it. `spec new
--translation` does not start from a remote source, because nothing is copied out of a remote:
copy the document into this repository first, or translate it in its own repository. `spec doctor` lists every remote,
its directory and whether it is present.

A document can pin a remote document by content when it depends on that exact text:
`spec set <id> --pin org:guide:root/setup` records `pins: {org:guide:root/setup: sha256:…}`, the
same body hash as a translation's `source_rev`. L7 warns when the remote body has changed; review
the change, then pin again. `spec set <id> --unpin org:guide:root/setup` removes a pin. A pin of a document that does not exist there is an error, and an
absent remote directory is skipped.

## Lifecycle

| rule | checks that |
| --- | --- |
| **T1** | superseded documents have exactly one successor |
| **T2** | status only moves forward |
| **T3** | sealed documents keep their body |
| **T4** | an edited body moves its updated date |
| **T5** | exceptions are reviewed by their date |
| **D1** | changed code moves its governing document (staged and ranged runs only; a warning) |

A document that departs from another on purpose names it with `rel.exception_of` and sets
`review_by: YYYY-MM-DD`. T5 warns when `review_by` is missing or not a date, and once the date has
passed (UTC). Renew it with `spec set <id> review_by=<date>` (set refuses a value that is not a date). When the
rule is kept again, retire the exception with `status=rejected`, or supersede it with a newer one.
Superseded and rejected exceptions are not checked. An exception that names itself is refused by
`spec set` and warned by T5. `spec audit --kind exception` lists the overdue ones. A local exception
lives in `.spec.local`, so it stays out of the shared repository.

T5 is a warning by default. To fail the check on an overdue exception, raise it in `spec.toml`:

```toml
[check.severity]
T5 = "error"
```

D1 runs only with `--staged` or `--changed`. Each changed file that is not a document is matched
against the explicit `rel.governs` of active, committed documents; a document whose code changed
must change in the same range, or be named by a `change` note that does. Otherwise D1 warns with
the changed paths, including deleted files and the old paths of renames. Documents that govern only
through their owner directory are not asked. To clear it, update the document (or its `verified_at`)
or link a change note: `spec new change --title … --depends-on <id> --write`.

## Visibility

| rule | checks that |
| --- | --- |
| **V1** | no spec documents under public paths |
| **V2** | committed documents do not cite local ones |
| **V3** | external notes cite documents that exist |

## Placement and attachments

| rule | checks that |
| --- | --- |
| **P1** | documents live with the owner of the code they govern |
| **P2** | every attachment is registered |
| **P3** | repository attachments stay small |
| **E1** | recorded attachments carry a matching hash |
| **E2** | stored attachments exist in the store |

## Retired layouts

| rule | checks that |
| --- | --- |
| **Z1** | retired layouts and identifiers stay retired |

## Verification

| rule | checks that |
| --- | --- |
| **X1** | a source of truth's verify commands pass |
| **X2** | an active source of truth says how it is verified |

## Generated documents

| rule | checks that |
| --- | --- |
| **G1** | generated documents match their generator |

## Guards

| rule | checks that |
| --- | --- |
| **U1** | guards pass |
| **U2** | documents hold no secrets (the built-in check `spec export` runs; `spec guard secrets`), repository text attachments included; a warning in local documents |

## Agent assets

| rule | checks that |
| --- | --- |
| **A1** | agent assets have the frontmatter their tools read |
| **A2** | asset names are unique within a tool |
| **A3** | paths and spec commands in agent assets exist |
| **A4** | agent assets use repository paths, not machine paths |

A3 does not read URLs, ids, globs, placeholders such as `<id>`, or git refs such as `origin/main` and `refs/heads/main` as paths. A4 reports home directories (`/Users/<name>`, `/home/<name>`, `C:\Users\<name>`) but not the hosted CI runner's `/home/runner/`, which is the same on every run; `/home/runner2/` is still reported.

## Translations

| rule | checks that |
| --- | --- |
| **R1** | translations are current with their source |
| **R2** | translations are where the `[translations]` layout expects them |

A translation names its source with `rel.translation_of` and records `source_rev`, a hash of the source body (`sha256:` and 12 hex digits) with frontmatter removed and whitespace collapsed, so it needs no git history and reformatting does not count. For a bundle, the hash covers the main file and then its chapters in path order. When the source body changes, R1 warns until the translation is updated and `spec set <id> --source-rev` records the source again. When git still has a commit with the recorded revision, the warning names the sections that changed since, for example `in Install (1 paragraph changed), Support (new)`; chapter sections are prefixed with the chapter file name. A source that no longer exists is an L3 finding. `spec census --translations` lists every translation's state, the changed sections of those behind, and the sources each language lacks. It exits 1 when anything is behind or missing; `--fail-on stale` fails only on translations that are behind or unrecorded, and `--fail-on none` reports without failing.

R1 also checks the values. `lang` must be a language tag (BCP 47, such as `ko`, `ja`, `pt-BR`). `source_rev` must be `sha256:` and 12 hex digits; `spec set` refuses other values. One source should have one translation per language. A translation must name an original: naming itself or another translation is an error, and the message names the original to use. `spec census --translations` lists only originals as sources. A translation's frontmatter looks like this:

```yaml
lang: ko
rel:
  translation_of: [guide:shop/setup]
source_rev: sha256:3f2a9c0b1d4e
```

A translation can also be paired by place, without frontmatter, when `spec.toml` sets `[translations] layout`. With `layout = "suffix"`, `guide:shop/setup-ko` is the `ko` translation of `guide:shop/setup`: same type and spec directory, slug ending in `-<lang>`. With `layout = "dir"`, the translation has the source's slug under an owner in a `<lang>` directory of the source's owner, for example `services/shop/ko/.spec/guide/setup.guide.md`; that directory must be an owner (`[[owners.kinds]] pattern = "services/*/ko"`). `<lang>` is one of `languages`. Timestamp-keyed translations keep their own timestamp and are paired by slug. `rel.translation_of` and `lang` in frontmatter win over the layout. R2 reports a document placed as a translation with no source, or more than one, in the matching place; `rel.translation_of` that names another document than the layout; and `lang` that differs from the language of its place. R2 warns; `strict = true` makes it an error. `spec fix --translations` previews writing `rel.translation_of` and `lang` into every pair the layout infers, and `--write` applies it.

`spec new --translation <lang> --from <id>` starts a translation: it takes the source's type, gets its own key, is placed by the layout (`<slug>-<lang>` beside the source without one), copies the source body (and the chapters and files of a bundle), and records `lang`, `rel.translation_of` and `source_rev`. It starts as `draft` where the type has that status. The tag in the name is lower case (`setup-pt-br`); `lang` keeps the BCP 47 form (`pt-BR`). A source that is itself a translation is refused with the original's id.

## Numbers

| rule | checks that |
| --- | --- |
| **N1** | counts in spec:count markers are current |

## Hidden characters

| rule | checks that |
| --- | --- |
| **H1** | no invisible characters in documents, declared agent assets, or well-known agent files |

Well-known agent files are checked even when they are not declared: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`, `.agents/skills/*/SKILL.md`, `.claude/skills/*/SKILL.md`, `.claude/agents/*.md`, `.claude/commands/*.md`, and `.cursor/rules/*.mdc`. Joiners used inside emoji (U+200C, U+200D) are allowed. Output never prints the characters themselves: messages, fixes, previews, and editor quick fixes show them as `\u{XXXX}`, and in an editor the warning covers just the one character. `spec fix` shows each changed line before and after.

A4 (machine-local paths) checks the same files as H1: declared agent assets and the well-known agent files, each once.

Plugin rules are named `x.<plugin>.<id>` and are explained the same way.
