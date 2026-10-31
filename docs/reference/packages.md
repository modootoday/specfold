---
title: Packages
description: The crates and the npm package specfold is published as, with their registries.
layout: reference
packages_title: "Published packages"
packages_compact: true
packages_position: after
packages:
  - {name: "specfold", version: "0.3.0", text: "The `specfold` and `spec` commands, built from source by `cargo install specfold`.", url: "https://crates.io/crates/specfold", links: [{label: "crates.io", url: "https://crates.io/crates/specfold"}, {label: "Source", url: "https://github.com/modootoday/specfold/tree/main/crates/specfold-cli"}]}
  - {name: "specfold-core", version: "0.3.0", text: "The library behind the commands: documents, rules and the graph.", url: "https://crates.io/crates/specfold-core", links: [{label: "crates.io", url: "https://crates.io/crates/specfold-core"}, {label: "docs.rs", url: "https://docs.rs/specfold-core"}, {label: "Source", url: "https://github.com/modootoday/specfold/tree/main/crates/specfold-core"}]}
  - {name: "@modootoday/specfold", version: "0.3.0", text: "The prebuilt binary for your platform, through npm optional dependencies.", url: "https://www.npmjs.com/package/@modootoday/specfold", links: [{label: "npm", url: "https://www.npmjs.com/package/@modootoday/specfold"}, {label: "Source", url: "https://github.com/modootoday/specfold/tree/main/npm"}]}
---

To install one, see [Install](../start/install.md). Each version is the latest published release.

- `specfold` 0.3.0: the `specfold` and `spec` commands, built from source by `cargo install specfold`.
  [crates.io](https://crates.io/crates/specfold),
  [Source](https://github.com/modootoday/specfold/tree/main/crates/specfold-cli)
- `specfold-core` 0.3.0: the library behind the commands: documents, rules and the graph.
  [crates.io](https://crates.io/crates/specfold-core), [docs.rs](https://docs.rs/specfold-core),
  [Source](https://github.com/modootoday/specfold/tree/main/crates/specfold-core)
- `@modootoday/specfold` 0.3.0: the prebuilt binary for your platform, through npm optional dependencies.
  [npm](https://www.npmjs.com/package/@modootoday/specfold),
  [Source](https://github.com/modootoday/specfold/tree/main/npm)

> [!NOTE]
> All three packages share one version. A release publishes the crates and the npm package
> together, and the [GitHub release](https://github.com/modootoday/specfold/releases/latest) of
> that version holds the prebuilt archives.
