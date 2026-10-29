---
title: Editing documents
description: Create, change, edit and move documents from the command line.
redirect_from: [/editing/]
---

# Editing documents

> [!TIP]
> Writes are previews until `--write`, except `new` and `set`, which write directly and accept
> `--dry-run`. Sealed and closed documents are refused.

## Create

```console
$ spec new plan --governs services/shop --title "Checkout launch"
$ spec new skill --title review --description "Review a change"
```

## Front matter

```console
$ spec set <id> status=active
$ spec set <id> tags+=money
```

## Bodies

`spec edit` replaces or appends to one section or the whole body, and bumps `updated`.

```sh
spec edit <id> --section Notes --text "Ship on Friday." --write
spec edit <id> --section Notes --write < notes.md
cat body.md | spec edit <id> --write
spec edit <id> --new-section Risks --after Plan --text "None yet." --write
spec edit <id> --row "Login" --cell Status=done --write
```

## Move and fix

`spec mv <id> --slug <new>` moves a document and rewrites every reference to it. `spec fix`
applies fixes that need no judgement; `spec fix --rule I2 --pick <id>` resolves a duplicate alias.
