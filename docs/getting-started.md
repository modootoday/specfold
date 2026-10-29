---
title: Getting started
description: Install spec, write a starter spec.toml and create a first document.
nav_order: 0
---

# Getting started

## Install

```sh
cargo install specfold                        # with Rust
npm install --save-dev @modootoday/specfold   # in a JavaScript project; run with npx spec
```

## First document

```console
$ spec init --owner 'services/*={1}' --write
$ spec new sot --governs services/shop/src/cart --title "Cart totals"
$ spec context services/shop/src/cart/total.rs
$ spec check
$ spec agents --for claude
```

`spec init` writes a starter `spec.toml`. `spec new` stores the document with the owner of the
code it governs. `spec context` lists the documents that govern a path, and `spec check` runs
the rules. `spec agents` prints the lines to put in an agent's instruction file.

## Next

- [Concepts](concepts.md): document types, ids, statuses and relations.
- [Editing documents](editing.md): change documents from the command line.
- [Commands](commands.md): every command.
