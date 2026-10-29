# Working on specfold

- **Generic only.** specfold is used by unrelated projects. Code, tests, fixtures, docs and commit
  messages name no real project, company, product or person. Fixtures use invented repositories
  (`services/shop`, `libs/auth`). Anything project-specific belongs in that project's `spec.toml`.
- **One responsibility per module.** A module file assembles (`mod` lines and re-exports); its
  directory holds the parts. Do not add unrelated code to an existing module; extend the structure.
  Inside a domain, data types, read-only queries and changes live in separate files.
- **The CLI contract is tested.** Every command keeps C1–C10: described by `spec help --json`, no
  prompts, text or `--json` with `schema`, `exit` and `next`, fixes on findings, previews before
  writes, bounded lists, one resolver, current documents by default, meaningful exit codes, a
  next command. `tests/cli_contract.rs` checks them for every command; add new commands there.
- **Dependencies are few and well known.** No network or TLS crates: stores and plugins are
  external commands. Add a dependency only with a reason in the pull request.
- **Before a commit:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- **Generated files follow the commands.** After changing a command, flag, help text or example,
  regenerate what the CLI writes and commit it with the change: `spec help --llms > llms.txt`,
  and the `<!-- spec:commands:begin/end -->` blocks in `README.md` and
  `docs/reference/commands.md` from `spec help --markdown`. `tests/docs.rs` fails on drift.
- **Comments say why**, in English, at most three lines. Rule explanations live in each rule's
  `explain` text, which is the documentation `spec explain` prints.
- **No design records here.** This repository publishes code; it holds no `.spec`, ADR or
  decisions directory (`spec check` refuses one through `[forbid]`). Explain a change in its
  commit message and in the code's own comments and rule texts.
- **Before pushing:** `spec guard public-surface --range origin/main..HEAD` with
  `SPECFOLD_PRIVATE_TERMS` set by the maintainer.
