---
title: Getting started
description: Install spec, write a starter spec.toml and create a first document.
nav_order: 0
---

# Getting started

## Install

There are three ways to install. Each gives you two commands, `specfold` and its short alias
`spec`.

### npm

```sh
npm install --save-dev @modootoday/specfold
npx spec --version
```

npm downloads only the build for your machine through platform-specific optional
dependencies. Do not install with `--omit=optional` or `--no-optional`; if a command reports
that a platform package is missing, install again without them.

### cargo

```sh
cargo install specfold
```

This builds from the crate on crates.io and needs a Rust toolchain.

### Prebuilt binary

Download the archive for your platform from the
[latest GitHub release](https://github.com/modootoday/specfold/releases/latest). Each archive
holds `specfold` and `spec` (with `.exe` on Windows), the README and the licences.

| platform | target | archive |
| --- | --- | --- |
| Linux x64 (glibc 2.35+) | `x86_64-unknown-linux-gnu` | `specfold-x86_64-unknown-linux-gnu.tar.gz` |
| Linux arm64 (glibc 2.35+) | `aarch64-unknown-linux-gnu` | `specfold-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Intel | `x86_64-apple-darwin` | `specfold-x86_64-apple-darwin.tar.gz` |
| macOS Apple silicon | `aarch64-apple-darwin` | `specfold-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `x86_64-pc-windows-msvc` | `specfold-x86_64-pc-windows-msvc.zip` |

glibc 2.35 covers Ubuntu 22.04 and newer. Every archive has a `.sha256` file next to it.
Check the download before you unpack it:

```sh
sha256sum -c specfold-x86_64-unknown-linux-gnu.tar.gz.sha256      # Linux
shasum -a 256 -c specfold-aarch64-apple-darwin.tar.gz.sha256      # macOS
tar -xzf specfold-x86_64-unknown-linux-gnu.tar.gz
```

On Windows, compare the output of
`Get-FileHash specfold-x86_64-pc-windows-msvc.zip -Algorithm SHA256` with the `.sha256` file.
Put the unpacked directory on your `PATH`.

## First document

```console
$ spec init --owner 'services/*={1}' --write
$ spec new sot --governs services/shop/src/cart --title "Cart totals"
$ spec context services/shop/src/cart/total.rs
$ spec check
$ spec agents --for claude
```

`spec init` writes a starter `spec.toml`. `spec new` stores the document with the owner of the
code it governs. `spec context` lists the documents that govern a path, and `spec check` runs
the rules. `spec agents` prints the lines to put in an agent's instruction file.

## Next

- [Concepts](concepts.md): document types, ids, statuses and relations.
- [Editing documents](editing.md): change documents from the command line.
- [Commands](commands.md): every command.
