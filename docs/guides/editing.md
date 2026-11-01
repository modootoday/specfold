---
title: Editing documents
description: Create, change, edit and move documents from the command line.
redirect_from: [/editing/]
---

# Editing documents

> [!TIP]
> Every write command, `new` and `set` included, previews until `--write` (`--dry-run` previews
> too); the preview ends with the same command and `--write`. Sealed and closed documents are
> refused.

## Create

```console
$ spec new plan --governs services/shop --title "Checkout launch" --write
$ spec new skill --title review --description "Review a change" --write
```

A title is one line: `new --title` and `set title=` refuse a line break (exit 2).

## Front matter

```console
$ spec set <id> status=active --write
$ spec set <id> tags+=money --write
```

## Bodies

`spec edit` replaces or appends to one section or the whole body, and bumps `updated`. The
examples use the plan created above, whose sections are `Goal`, `Steps` and `Record`; a section
that does not exist is refused with the list of sections and a `--new-section` command.

```sh
spec edit <id> --section Goal --text "Ship on Friday." --write
printf '| Step | status |\n|---|---|\n| Login | todo |\n' > steps.md
spec edit <id> --section Steps --write < steps.md
spec edit <id> --row "Login" --cell status=done --write
spec edit <id> --new-section Risks --after Steps --text "None yet." --write
cat body.md | spec edit <id> --write
```

The new plan has no table, so the second and third lines first put one in `Steps`. `--row` edits a
Markdown pipe table in the document: it picks the row whose first cell is `Login` and `--cell` sets
the cell under the `status` column (column names match case-insensitively); without a table it is
refused (exit 2). The last line replaces the whole body with `body.md`, sections included.

A preview lists each file an update changes, with its changed lines as unified-diff hunks
(`@@ -old,n +new,m @@`, written as `git diff -U0` writes them: a count of 1 is left out, and an
empty side names the line before the change): changes with an unchanged line between them are
separate hunks, so the refreshed `updated:` line and the edited section show apart. A change too
large to compare line by line (over about four million line pairs) is shown as one hunk. In `--json`, each update item has
`line` (where the first hunk starts in the new text) and `hunks`, each with `line`, `old_line`,
`before` and `after`; `line` and `old_line` follow the same rule as the header.

## Move and fix

`spec mv <id> --slug <new>` moves a document and rewrites every reference to it. `spec fix`
applies fixes that need no judgement; `spec fix --rule I2 --pick <id>` resolves a duplicate alias.

## From another directory

`spec -C <dir> ...` runs as if started in `<dir>`. Relative path arguments
(`attach` files, `edit --file`, `import --map`, `export --out`, `audit --from`) are
read from `<dir>`, and every `next:` command keeps `-C <dir>`. Absolute paths stay as
they are.

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
