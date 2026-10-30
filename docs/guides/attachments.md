---
title: Attachments
description: Files that belong to a document, in the repository or an external store.
redirect_from: [/attachments/]
---

# Attachments

> [!TIP]
> `spec attach` registers files with a document and records their SHA-256. Small files stay in
> the repository; larger ones go to a store you configure.

## Commands

```console
$ spec attach <id> results.csv --write
$ spec files <id>
$ spec fetch <id>
```

## Stores

A store is a directory, or a command you already trust (rclone, aws, gsutil, wrangler). specfold
contains no network code and never handles credentials. `[files] inline_max_bytes` sets the
limit for the repository.

A directory store:

```toml
[files]
inline_max_bytes = 1048576   # larger files go to the store

[files.store]
kind = "dir"
path = "~/specfold-store"    # `~` is expanded
```

A command store runs argv templates; `{src}` and `{dst}` are local paths, `{key}` is the object
key. `exists` is optional and succeeds (exit 0) when the object is there:

```toml
[files.store]
kind = "command"
put = ["rclone", "copyto", "{src}", "remote:bucket/{key}"]
get = ["rclone", "copyto", "remote:bucket/{key}", "{dst}"]
exists = ["rclone", "lsf", "remote:bucket/{key}"]
```

Every attached file is read by the built-in secrets check, the one `spec export` and rule U2
run. Before a file goes to the store, every guard that reads content (secrets, denied terms or
patterns, denied files) checks it as well. An error-level hit refuses the upload and names the
guard; `--allow-hit <guard>` lets that guard's hits through (`--allow-hit secrets` for the
built-in check). A secret in a file that stays in the repository is a warning at attach time.
Only the first 64 MiB of a file is read.

After that, rule U2 (in `spec check` and the commit hook) and `spec guard secrets` keep reading
the text files kept in the repository, so a secret added or left there later is an error in a
committed document and a warning in a local one. Files over `[files] scan_max_bytes` (1 MiB by
default) and binary files are not read; stored files were checked before upload. `spec export`
reads the same files for the documents it exports, and refuses the export on a hit.

## Rules

- **P2**: every attachment is registered.
- **P3**: repository attachments stay small.
- **E1**: recorded attachments carry a matching hash.
- **E2**: stored attachments exist in the store.
