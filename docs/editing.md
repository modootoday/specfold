---
title: Editing documents
description: Create, change, edit and move documents from the command line.
nav_order: 2
---

# Editing documents

Writes are previews until `--write`, except `new` and `set`, which write directly and accept
`--dry-run`. Sealed and closed documents are refused.

## Create

```sh
spec new plan --governs services/shop --title "Checkout launch"
spec new skill --title review --description "Review a change"   # agent assets, no id
```

## Frontmatter

```sh
spec set <id> status=active
spec set <id> tags+=money
```

## Bodies

`spec edit` replaces or appends to one section or the whole body and bumps `updated`.

```sh
spec edit <id> --section Notes --text "Ship on Friday." --write
spec edit <id> --section Notes --write < notes.md          # stdin, a pipe or a heredoc
cat body.md | spec edit <id> --write                        # the whole body
spec edit <id> --new-section Risks --after Plan --text "None yet." --write
spec edit <id> --row "Login" --cell Status=done --write     # one table row by its first cell
spec edit <id> --row "Logout" --append --text "Logout | todo" --write
```

## Move and rename

`spec mv <id> --slug <new>` moves a document and rewrites every reference to it.

## Fix

`spec fix` applies fixes that need no judgement; `spec fix --rule I2 --pick <id>` resolves a
duplicate alias.
