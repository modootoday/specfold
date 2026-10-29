---
title: FAQ
description: Short answers to common questions about specfold.
---

# FAQ

## What is specfold?

A command-line tool, `spec`, that keeps design, plan and maintenance documents in your
repository and checks their links and metadata.

## Does it need a server or network access?

No. `spec` is a single offline binary. Stores and plugins are external commands you configure.

## How do AI agents use it?

They run `spec` with `--json` to find the documents that govern a file, edit sections, and run
`spec check` before committing. See [Using with AI agents](guides/agents.md).

## Can I keep my current docs folder?

Yes. `spec import` moves an existing layout through a map, and old paths keep resolving. See
[Importing an existing layout](guides/import.md).

## Where do private notes go?

Into the git-ignored `[local]` directory, created with `spec new --local`. Rule **V2** stops
committed documents from citing them.

## What is the license?

MIT or Apache-2.0, at your option.
