---
title: specfold — docs that live with your code
description: "specfold keeps design, plan, and maintenance docs next to your code, linked as a graph and guarded by checks."
hero_art: graph
sections: [install, how, run, glance, next, faq]
---

# Your code moves. Your docs keep up.

**specfold** is a command-line tool, `spec`, that keeps design, plan and maintenance docs next to
the code they govern, links them as a graph and checks them on every change.

[Get started](start/index.md)

## Install it your way

Every method gives you two commands, `specfold` and its short alias `spec`.

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
sha256sum -c specfold-<target>.tar.gz.sha256
tar -xzf specfold-<target>.tar.gz
```

Prebuilt archives for Linux, macOS and Windows are on the
[latest release](https://github.com/modootoday/specfold/releases/latest). See
[Install](start/install.md) for every target.

## Three steps, then it runs by itself

1. **Write.** `spec new sot` stores a document with the owner of the code it governs.
2. **Link.** `rel.governs` ties it to code paths; other relations tie it to documents.
3. **Check.** `spec check` fails CI when a document drifts from the code or the rules.

## See it run

```console
$ spec context services/shop/src/cart/total.rs
sot:shop/cart-totals  Cart totals  [active]
$ spec check --summary
ok: 3 documents, 0 errors, 0 warnings
```

## At a glance

- **11** document types
- **36** rules, each explained by `spec explain`
- **5** prebuilt platforms
- **0** servers or network calls

## Where next

- **[Concepts](concepts/index.md)** — types, lifecycle, owners, the graph and checks, one page each.
- **[Guides](guides/agents.md)** — AI agents, CI, importing, editing and attachments.
- **[Reference](reference/commands.md)** — every command, rule and configuration key.

## Questions

**What is specfold?** A command-line tool, `spec`, that keeps design, plan and maintenance
documents in your repository and checks their links and metadata.

**Does it need a server or network access?** No. `spec` is a single offline binary. Stores and
plugins are external commands you configure.

**How do AI agents use it?** They run `spec` with `--json` to find the documents that govern a
file, edit sections, and run `spec check` before committing. More in the [FAQ](faq.md).
