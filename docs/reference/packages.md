---
title: Packages
description: The crates and the npm package specfold is published as, with their registries.
layout: reference
packages_title: "Published packages"
packages:
  - {name: "specfold", version: "0.3.0", text: "The `specfold` and `spec` commands, built from source by `cargo install specfold`.", url: "https://crates.io/crates/specfold", links: [{label: "crates.io", url: "https://crates.io/crates/specfold"}, {label: "Source", url: "https://github.com/modootoday/specfold/tree/main/crates/specfold-cli"}]}
  - {name: "specfold-core", version: "0.3.0", text: "The library behind the commands: documents, rules and the graph.", url: "https://crates.io/crates/specfold-core", links: [{label: "crates.io", url: "https://crates.io/crates/specfold-core"}, {label: "docs.rs", url: "https://docs.rs/specfold-core"}, {label: "Source", url: "https://github.com/modootoday/specfold/tree/main/crates/specfold-core"}]}
  - {name: "@modootoday/specfold", version: "0.3.0", text: "The prebuilt binary for your platform, through npm optional dependencies.", url: "https://www.npmjs.com/package/@modootoday/specfold", links: [{label: "npm", url: "https://www.npmjs.com/package/@modootoday/specfold"}, {label: "Source", url: "https://github.com/modootoday/specfold/tree/main/npm"}]}
---

Versions are the latest published release. All three move together: a release publishes the
crates and the npm package with the same version, and the
[GitHub release](https://github.com/modootoday/specfold/releases/latest) of that version holds
the prebuilt archives. To install, see [Install](../start/install.md).
