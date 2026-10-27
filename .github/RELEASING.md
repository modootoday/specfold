# Releasing

1. Bump `version` in the root `Cargo.toml` (both crates share it) and the `specfold-core`
   dependency version, run `cargo test`, and commit.
2. Optionally rehearse: run the Release workflow by hand. It builds every binary into a draft,
   runs `cargo publish --workspace --dry-run`, and deletes the draft.
3. Push the tag `v<version>`. The workflow builds the binaries into a draft release and waits for
   a reviewer to approve the `release` environment.
4. After approval it publishes `specfold-core` and `specfold` to crates.io and marks the release
   published.

## One-time setup

- **GitHub environment** `release` with required reviewers.
- **crates.io Trusted Publishing** for both crates: repository `modootoday/specfold`, workflow
  `release.yml`, environment `release`. crates.io allows this only for a crate that already
  exists, so the very first version of each crate is published once by a maintainer with an
  API token; every later version goes through the workflow without a stored token.
