---
title: Attachments
description: Files that belong to a document, in the repository or an external store.
nav_order: 4
---

# Attachments

Non-Markdown files belong to a document and are listed, with their SHA-256, in its `files`
frontmatter by `spec attach`. Small files stay in the repository; larger ones go to a store
configured in `spec.toml`: a directory, or a command you already trust (rclone, aws, gsutil,
wrangler …). specfold contains no network code and never handles credentials.

```sh
spec attach <id> results.csv --write
spec files <id>
spec fetch <id>
```
