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

## In an editor

`spec lsp` serves the findings of `spec check` as diagnostics over the Language Server Protocol
(stdio). It checks unsaved buffers, offers each `spec fix` replacement as a quick fix, and runs
only the `--write` fix commands it offered. It finds the repository from the editor's root, or
from `spec lsp --dir <path>`. Point any LSP client at it for Markdown files.

Neovim (0.11 or later):

```lua
vim.lsp.config("specfold", { cmd = { "spec", "lsp" }, filetypes = { "markdown" }, root_markers = { "spec.toml", ".git" } })
vim.lsp.enable("specfold")
```

Helix (`languages.toml`):

```toml
[language-server.specfold]
command = "spec"
args = ["lsp"]

[[language]]
name = "markdown"
language-servers = ["marksman", "specfold"]
```

VS Code: specfold ships no extension; use a generic LSP client extension with the command
`spec lsp` for Markdown files.
