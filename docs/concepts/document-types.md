---
title: Document types
description: The eleven built-in document types and what each one is for.
---

# Document types

> [!TIP]
> Pick the type by the question the document answers: what holds now (`sot`), why we chose
> (`adr`), what we will do (`plan`), or how to do it (`guide`, `runbook`).

## Types

There are <!-- spec:count rows="Types" -->11<!-- /spec:count --> built-in types.

| type | key | what it is |
| --- | --- | --- |
| sot | slug | a source of truth: the rule that holds now |
| adr | timestamp | a decision and the reason for it |
| plan | timestamp | a plan of work and its running record |
| research | timestamp | a study, benchmark or evaluation |
| evidence | timestamp | the cover of a bundle of measurements or raw outputs |
| audit | timestamp | an audit snapshot; its body is sealed once committed |
| page | slug | the specification of one screen or page |
| design | timestamp | a bundle of design boards |
| guide | slug | how to develop, operate or use something |
| runbook | slug | a step-by-step operational procedure |
| change | timestamp | one user-facing change for the release notes |

## Slug or timestamp

A **slug** key names something that stays: a rule, a page, a procedure. Its id never changes,
and the body is updated in place.

A **timestamp** key names something that happened at a moment: a decision, a study, a plan. A
new decision is a new document that supersedes the old one, so history stays readable.

## Which one to write

- The code must always behave a certain way: write a `sot`, and give it a `verify` command.
- You chose between options: write an `adr` and link the `sot` it shapes.
- Work spans several commits: write a `plan` with task items.
- You measured something: write `research`, and attach raw outputs as `evidence`.
- Someone repeats a procedure: write a `runbook`.

## Change the set

Projects can disable, change or add types in `spec.toml` under `[types.<name>]`: key, form,
statuses, fields, template and transitions. `spec schema <type>` prints a type's front matter.

```console
$ spec schema adr
$ spec new adr --title "Round totals half up" --write
```

Next, see how a document moves through its [lifecycle](lifecycle.md).
