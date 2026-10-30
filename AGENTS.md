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
- **CLI conventions CLI-1–CLI-17.** Write commands preview by default and apply only with `--write`;
  every command declares a risk level (read, write, destructive, external) in `spec help --json`;
  destructive or external actions need explicit confirmation when not on a terminal; data on stdout,
  diagnostics on stderr; honour NO_COLOR; one meaning per shared flag name; deprecated names stay as
  hidden aliases for at least one minor release; a write a safeguard refused exits 4. New
  commands add a row to `tests/cli_contract.rs`.
  The conventions are numbered CLI-n so they do not clash with the contracts C1–C10 above.
- **Code limits CLI-18–CLI-24.** Lints in `Cargo.toml` `[workspace.lints]`, the crate roots and
  `clippy.toml` enforce what the code already meets; the rest is checked in review against a
  baseline of existing exceptions, and new code adds none.
  - CLI-18: a source file stays under 500 lines, a test file under 1600; past that, split it into
    a directory.
  - CLI-19: a function stays under 120 lines, cognitive complexity 30, at most 7 arguments.
  - CLI-20: library code has no `unwrap()`; use `?` or `expect("why it holds")`. No `panic!`,
    `todo!`, `unimplemented!` or `unsafe` (denied); tests may unwrap and panic.
  - CLI-21: every file starts with a `//!` line; every public function, type, trait and constant
    has a `///` line (fields and variants when they need it).
  - CLI-22: test names are snake_case `behaviour_condition_result`, at most 80 characters, with a
    finding prefix (`c161_`) when they guard one; no `test_` prefix.
  - CLI-23: the core library prints nothing to stdout; output is the CLI's. The workspace keeps
    at most 20 direct dependencies.
  - CLI-24: errors are `anyhow` at the command boundary, read `what went wrong; next: <command>`,
    and get their exit code in one place; usage errors exit 2.
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
