---
name: specfold-documents
description: Read and write a repository's specfold documents with the spec CLI - spec context before changing code, spec new to create a source of truth, decision, plan, research, evidence, audit, page, design, guide or runbook where its owner keeps it, spec edit for body sections, spec set for status and relations, spec mv to move, rename or promote a local draft, spec attach for files, and spec find, show and graph to look things up. Use when a repository has a spec.toml and you are about to change code, record a decision, start or update a plan, or need to know which document rules some code. Not for setting specfold up (specfold-setup) or fixing check failures (specfold-checks).
metadata:
  tier: open
  level: L1
  domain: documentation
  install: optional
  keywords: "specfold, spec new, spec context, spec set, ADR, source of truth, plan, research, evidence, runbook, .spec"
  requires-bin: "spec"
---

# Working with specfold documents

Never create, rename or move a file under `.spec/` by hand, and never type a document's file
name. The CLI computes where a document lives and what it is called from its type, its owner
and the code it governs, and rewrites every reference when it moves.

## Before changing code

```sh
spec context path/to/the/code
```

It lists the documents that govern that code, with their verify commands. Read them; if the
change breaks a rule a source of truth states, either keep the rule or update the document in
the same change.

## Pick the type

| You are writing                                    | Type       |
| -------------------------------------------------- | ---------- |
| The rule that holds now                            | `sot`      |
| A decision and why it was made                     | `adr`      |
| Work to do, and its running record                 | `plan`     |
| A study, benchmark or evaluation                   | `research` |
| Raw outputs or measurements other documents cite   | `evidence` |
| A security or compliance snapshot (sealed)         | `audit`    |
| One screen or page                                 | `page`     |
| Design boards                                      | `design`   |
| How to develop, operate or use something           | `guide`    |
| Step-by-step operational procedure                 | `runbook`  |

`spec schema <type>` shows its frontmatter. A project may add or change types in `spec.toml`.

## Create

```sh
spec new adr --governs services/shop/src/db --title "Use Postgres for orders"
spec new plan --governs services/shop --title "Checkout v2"
spec new adr --from adr:shop/20250101000000 --title "Move orders to a queue"   # supersedes
```

`--governs` decides the owner. Use `--local` for a draft that stays out of git, and
`--dry-run` to see the path first. The new document has the sections its type requires.
`spec mv <id> --tier committed --write` later promotes a local draft into the committed tree
(`--tier local` takes one back out).

## Edit the body

```sh
spec edit adr:shop/20250101000000 --section Decision --text "Use Postgres." --write
spec edit plan:shop/20260101000000 --new-section Risks --after Goal --text "- none yet" --write
```

`--section` changes an existing heading; a missing required section (rule S7) is added with
`--new-section`. Without `--write` it previews.

## Change status and relations

```sh
spec set plan:shop/20260101000000 status=active
spec set plan:shop/20260101000000 rel.implements+=sot:shop/cart
spec set sot:shop/cart tags-=draft
```

Statuses move forward only: `draft`, `active`, then `superseded`, `archived` or `rejected`.
Relations are written once, on the document that makes them: `rel.governs`, `rel.supersedes`,
`rel.implements`, `rel.evidence`, `rel.depends_on`, `rel.translation_of`, `rel.exception_of`. In the body, refer to
another document as `[[<id>]]` or `[[<id>#<anchor>]]`, never by path. A document in another
repository listed under `[remotes.<name>]` in spec.toml is `<name>:<id>` (`[[org:adr:root/…]]`);
read it with `spec show <name>:<id>`, and change it in that repository, not from here.
A translation may name such a source, and `[[<name>:<id>]]` in READMEs and code comments is
checked too. `spec doctor` shows which remotes are on disk. When this document relies on the exact
text of a remote one, pin it: `spec set <id> --pin <name>:<id>`; L7 warns when it changes. Before changing a shared document, `spec graph consumers <id>` lists who
relies on it in the remotes on disk.

An exception to a rule or decision is its own document: `rel.exception_of` names what it departs
from, and `review_by: YYYY-MM-DD` says when to look again. Keep a personal or temporary one in
`.spec.local`. When T5 says the date has passed, ask whether the exception still holds; renew the
date only with a reason, otherwise archive it.

A translation is its own document that names its source and language, and records the source
body it was made from:

```sh
spec set guide:shop/setup-ko lang=ko rel.translation_of+=guide:shop/setup
spec set guide:shop/setup-ko --source-rev      # after the text matches the source
```

When the source changes, R1 warns; update the translation, then run `--source-rev` again.

The frontmatter then holds `lang: ko`, `rel: {translation_of: [guide:shop/setup]}` and
`source_rev: sha256:3f2a9c0b1d4e`. `lang` is a language tag (BCP 47); a translation names the
original, never another translation; one translation per language per source.

To start one, `spec new --translation ko --from guide:shop/setup` copies the source body and
records all three. With `[translations] layout` in spec.toml, a translation in its place
(`setup-ko` beside the source, or under a `ko` owner for `layout = "dir"`) is paired without
frontmatter; `spec fix --translations --write` writes the pair down.

## Move, attach, look up

```sh
spec mv sot:shop/cart --slug checkout-cart --write
spec attach evidence:root/20260101000000 run.jsonl --write
spec find "rate limit" --type adr
spec show adr:shop/20250101000000 --rel
spec graph impact sot:shop/cart
```

`spec show <id> --section <heading>` prints one section instead of the whole document, and
`--max-bytes <n>` caps a list's output to what fits (the rest is left for `--offset`).
`spec resolve` turns an old path, alias or slug into the current id.

## Before committing

```sh
spec check --staged
spec fix --staged --write      # mechanical fixes only
```

What a rule means: `spec explain <rule>`.
