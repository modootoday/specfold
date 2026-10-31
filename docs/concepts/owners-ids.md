---
title: Owners and ids
description: Owners decide where documents live; ids are computed from that place.
---

# Owners and ids

> [!TIP]
> Each owner (the repository root, an app, a package) keeps its documents in its own `.spec/`.
> A document's id, `<type>:<owner>/<key>`, is computed from where it lives.

## Owners

Owners are found through package manifests (`package.json`, `Cargo.toml` and others) or
`[owners]` patterns in `spec.toml`. A pattern such as `services/*={1}` makes every directory
under `services/` an owner named after the directory.

`spec new` stores a document with the owner of the code it governs. A document without
`rel.governs` stays with the root owner. Rule **P1** reports documents that live with the wrong
owner.

```console
$ spec owners
$ spec new sot --governs services/shop/src/cart --title "Cart totals" --write
```

## Ids

| part | example | from |
| --- | --- | --- |
| type | `sot` | the document's type |
| owner | `shop` | the owner directory |
| key | `cart-totals` | the slug or UTC timestamp |

So the file `services/shop/.spec/sot/cart-totals.sot.md` has the id `sot:shop/cart-totals`. A
document with chapters or attachments is a directory with `index.<type>.md` instead.

Rule **S2** checks that the id matches the path, and **I1** that ids are unique across every root.

## Moving

`spec mv <id> --slug <new>` moves or renames a document and rewrites every reference to it.
`spec import` brings in a whole layout and writes a frozen alias table, so old ids keep
resolving (rules **I2**, **I3**).

## Public paths

Published paths, such as a package directory, may hold no spec document (rule **V1**). The
`[public]` table names the private owner their documents go to instead.

Next: how documents connect, in [The graph](graph.md).
