---
title: Importing an existing layout
description: Bring documents in from another layout through an import map.
redirect_from: [/import/]
---

# Importing an existing layout

> [!TIP]
> Describe the old layout in an import map, review the plan, then apply it. Old ids and paths
> keep resolving through a frozen alias table.

## Steps

1. **Write a map.** Patterns, types, owners, key rules, status mappings and redirect stubs.
2. **Review the plan.** `spec import --map import.toml` changes nothing.
3. **Apply.** `--apply --write` moves everything and writes the alias table.

```console
$ spec import --map import.toml
$ spec import --map import.toml --apply --write
$ spec check
```

## After the move

Rules **I2** and **I3** keep the alias table honest: aliases point at one document, and the
table gains no rows. `[forbid]` keeps retired layouts from coming back (rule **Z1**).
