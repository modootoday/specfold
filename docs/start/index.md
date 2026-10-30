---
title: "Tutorial: three steps"
description: Write a first document, link it to code and check it, in three steps.
redirect_from: [/getting-started/]
---

# Tutorial: three steps

> [!TIP]
> In five commands you get a `spec.toml`, a document that governs a code path, and a passing
> `spec check`. Install first: [Install](install.md).

## The steps

1. **Write.** `spec init` writes a starter `spec.toml`; `spec new` stores a document with the
   owner of the code it governs.
2. **Link.** `--governs` records the code path. `spec context` now lists the document for any
   file under it.
3. **Check.** `spec check` runs the rules. `spec agents` prints the lines for an agent's
   instruction file.

## Run it

```console
$ spec init --owner-kind 'services/*={1}' --write
$ spec new sot --governs services/shop/src/cart --title "Cart totals"
$ spec context services/shop/src/cart/total.rs
$ spec check
$ spec agents --for claude
```

## What happened

The `--owner-kind` pattern makes every directory under `services/` an owner, so the document lands in
`services/shop/.spec/sot/`. Its id, `sot:shop/cart-totals`, comes from that place.

`spec context` answers the question an agent or reviewer asks first: which documents must I read
before changing this file?

## Next

- [Document types](../concepts/document-types.md): what to write besides a `sot`.
- [Lifecycle](../concepts/lifecycle.md): how a document becomes `active`.
- [Editing documents](../guides/editing.md): change documents without hand-editing front matter.
