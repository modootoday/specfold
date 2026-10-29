---
title: specfold — docs that live with your code
description: "specfold keeps design, plan, and maintenance docs next to your code, linked as a graph and guarded by checks."
---

## What specfold does

specfold is a command-line tool, `spec`, for the documents that explain a codebase: sources of
truth, plans, research, runbooks and guides. Each document is a Markdown file with front matter,
stored next to the code its owner governs.

- `spec context <path>` lists the documents to read before changing a file.
- `spec new`, `spec set` and `spec edit` change documents without hand-editing front matter.
- `spec check` runs the rules in CI and before each commit.

Start with [Getting started](getting-started.md) or read the [Concepts](concepts.md).
