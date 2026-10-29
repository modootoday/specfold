---
title: Install
description: Install spec with npm, cargo or a prebuilt binary, and verify the download.
---

# Install

> [!TIP]
> Pick one method. Each gives you two commands, `specfold` and its short alias `spec`.

## Choose a method

### npm

```sh
npm install --save-dev @modootoday/specfold
npx spec --version
```

### cargo

```sh
cargo install specfold
```

### Binary

```sh
sha256sum -c specfold-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf specfold-x86_64-unknown-linux-gnu.tar.gz
```

npm downloads only the build for your machine through platform-specific optional dependencies.
Do not install with `--omit=optional` or `--no-optional`. cargo builds from crates.io and needs a
Rust toolchain.

## Prebuilt targets

Download from the [latest GitHub release](https://github.com/modootoday/specfold/releases/latest).
Each archive holds `specfold` and `spec` (`.exe` on Windows), the README and the licences.

| platform | target | archive |
| --- | --- | --- |
| Linux x64 (glibc 2.35+) | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Linux arm64 (glibc 2.35+) | `aarch64-unknown-linux-gnu` | `.tar.gz` |
| macOS Intel | `x86_64-apple-darwin` | `.tar.gz` |
| macOS Apple silicon | `aarch64-apple-darwin` | `.tar.gz` |
| Windows x64 | `x86_64-pc-windows-msvc` | `.zip` |

Archive names are `specfold-<target>` plus the extension. glibc 2.35 covers Ubuntu 22.04 and newer.

## Verify the download

Every archive has a `.sha256` file next to it.

```sh
sha256sum -c specfold-x86_64-unknown-linux-gnu.tar.gz.sha256   # Linux
shasum -a 256 -c specfold-aarch64-apple-darwin.tar.gz.sha256   # macOS
```

On Windows, compare `Get-FileHash specfold-x86_64-pc-windows-msvc.zip -Algorithm SHA256` with the
`.sha256` file. Put the unpacked directory on your `PATH`.
