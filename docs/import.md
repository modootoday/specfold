---
title: Importing an existing layout
description: Bring documents in from another layout through an import map.
nav_order: 6
---

# Importing an existing layout

`spec import --map import.toml` reads files from any other layout through a map of patterns,
types, owners, key rules, status mappings and redirect stubs. It prints a reviewable plan and,
with `--apply --write`, moves everything and writes a frozen alias table so old ids and paths
keep resolving until they are rewritten.
