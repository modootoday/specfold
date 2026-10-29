---
title: Times and tasks
description: How specfold writes and reads times, and how task items are counted.
---

# Times and tasks

> [!TIP]
> specfold writes every time in UTC and reads common forms, including a few named zones.
> Checkbox items in a body are counted as tasks.

## Written times

Timestamp keys (`YYYYMMDDHHMMSS`) are UTC. specfold writes `created` and `updated` as RFC 3339
times in UTC, such as `2026-10-30T02:21:00Z`. Times are compared by their UTC day.

## Read times

| form | example | accepted |
| --- | --- | --- |
| date | `2026-10-30` | yes |
| RFC 3339 with offset | `2026-10-30T11:21:00+09:00` | yes |
| named zone | `2026-10-30 11:21 KST` | UTC, GMT, KST, JST only |
| ambiguous zone | `2026-10-30 11:21 CST` | refused |

Named zones are a reading aid only. Rule **S5** checks that dates are valid and that `updated` is
not before `created`.

## Task items

Checkbox items (`- [ ]`, `- [x]`) in a body are tasks.

```console
$ spec show plan:shop/20261029190949
$ spec show plan:shop/20261029190949 --tasks
```

`spec show` reports a document's progress; `--tasks` lists each item with its file and line.
`spec audit` reports plans whose tasks are all ticked but that are still active, and archived
plans with tasks still open.

## Related

- [Lifecycle](lifecycle.md): when a plan should move to `archived`.
- [Checks](checks.md): the audit kinds.
