# specfold

[![release](https://img.shields.io/badge/release-latest-blue)](https://github.com/modootoday/specfold/releases/latest)
[![source](https://img.shields.io/badge/source-GitHub-black)](https://github.com/modootoday/specfold)
[![tests](https://github.com/modootoday/specfold/actions/workflows/cross-os-test.yml/badge.svg)](https://github.com/modootoday/specfold/actions/workflows/cross-os-test.yml)

<!-- spec:description:begin -->
One CLI for a repository's design, planning and maintenance documents, built for people and AI agents.
<!-- spec:description:end -->

specfold types the documents a project writes about itself (rules, decisions, plans, research,
evidence, audits, page specs, guides, runbooks), keeps each beside the code it governs, and checks
them from one command, `spec`.

## Contents

[Why](#why) · [Install](#install) · [Quick start](#quick-start) · [Concepts](#concepts) ·
[Commands](#commands) · [Using with AI agents](#using-with-ai-agents) · [Guides](#guides) ·
[Exit codes and JSON](#exit-codes-and-json) · [Contributing](#contributing) · [License](#license)

## Why

- **Documents live with the code they govern.** Each owner (the root, an app, a package) keeps
  its documents in its own `.spec/`; `spec new --governs <path>` finds the owner for you.
- **One definition, a derived graph.** Types, statuses and checks come from one `spec.toml`;
  references, supersede chains and governed code are computed on every run, never stored.
- **Built for AI agents first.** No prompts, stable `--json`, bounded output, a fix on every
  finding and a suggested next command. `spec mcp` serves the same commands as MCP tools.

## Install

Each method gives you two commands, `specfold` and its short alias `spec`.

```sh
cargo install specfold                        # with Rust
npm install --save-dev @modootoday/specfold   # in a JavaScript project; run with npx spec
```

Binaries for Linux, macOS and Windows: [latest release](https://github.com/modootoday/specfold/releases/latest).
To update, run the same command again.

## Quick start

In a repository with apps under `services/`:

<!-- spec:quickstart:begin -->
```console
$ spec init --owner-kind 'services/*={1}' --write
written
$ spec new sot --governs services/shop/src/cart --title "Cart totals"
created sot:shop/cart-totals  services/shop/.spec/sot/cart-totals.sot.md
$ spec context services/shop/src/cart/total.rs
  sot:shop/cart-totals               draft     Cart totals
$ spec check
0 error(s), 0 warning(s) in 1 of 1 documents
$ spec agents --for claude
# add to CLAUDE.md
```
<!-- spec:quickstart:end -->

`init` writes `spec.toml`, `new` creates a source of truth for the cart code, `context` shows
what governs a file before you change it, `check` runs the rules, and `agents` prints the lines
to put in your agent's instruction file (output trimmed; the tests check each line shown). Run `spec check --staged` before each commit, or wire
it in with `spec hook print --for pre-commit`.

## Concepts

Each document has a type: `sot` (the rule that holds now), `adr` (a decision), `plan`, `research`,
`evidence`, `audit`, `page`, `design`, `guide`, `runbook` and `change`. `spec schema` lists them.

Ids are `<type>:<owner>/<key>` and come from where a document lives. Statuses move forward only
(`draft`, `active`, `superseded`, `archived`, `rejected`), and relations such as `rel.governs`
and `rel.supersedes` are written once, on the document that makes them. Details, including
times and task items: [docs/concepts/index.md](docs/concepts/index.md).

## Commands

<!-- spec:commands:begin -->

| command | what it does |
| ------- | ------------ |
| `spec context` | The documents that govern code paths: read this before changing code |
| `spec find` | Find documents by words, type, owner, status or tag (also `spec list`, `spec ls`) |
| `spec show` | Frontmatter, outline, one section, or relations of a document |
| `spec resolve` | Resolve an id, alias, path or slug to one document |
| `spec graph` | Graph queries: impact, chain, governs, consumers; or the whole graph |
| `spec owners` | Owners, their documents and where public owners store theirs |
| `spec assets` | Agent skills, rules, subagents and instruction files declared under [[assets]] |
| `spec log` | The history of a document's status and relations, from git |
| `spec new` | Create a document at the place its owner and type decide |
| `spec set` | Change frontmatter with the lifecycle enforced |
| `spec edit` | Replace or append to a section or the whole body; previews until `--write` |
| `spec mv` | Move or rename a document and rewrite every reference to it |
| `spec fix` | Apply fixes that need no judgement |
| `spec sync` | Copy asset sources over the [[assets]] roots that declare copy_of; previews until `--write` |
| `spec attach` | Register files with a document, in the repository or the store |
| `spec fetch` | Download a document's stored attachments, verified by hash |
| `spec files` | A document's attachments and where each is kept |
| `spec init` | Write a starter spec.toml, or upgrade an existing one |
| `spec import` | Bring documents from another layout in through an import map |
| `spec check` | Run the rules |
| `spec verify` | Run sot verify commands |
| `spec guard` | Run guards over files and commit messages |
| `spec audit` | Diagnose what passes the rules yet needs attention; build review packs; record audits |
| `spec census` | Counts of documents, references and attachments |
| `spec doctor` | Read-only setup diagnosis: configuration, conflict markers, ignored local documents |
| `spec explain` | Why a rule exists and how to fix it; or the configuration keys |
| `spec schema` | Document types and their frontmatter; or output schema names |
| `spec help` | Commands and arguments; with `--json`, the whole command tree |
| `spec agents` | The lines to put in an agent's instruction file |
| `spec hook` | Hook snippets for git, pre-commit, lefthook or husky |
| `spec plugins` | Configured plugins and what they provide |
| `spec export` | Export as a Markdown tree, or through an exporter plugin |
| `spec mcp` | Serve these commands as MCP tools over stdio |
| `spec lsp` | Serve check findings as editor diagnostics (Language Server Protocol over stdio) |
<!-- spec:commands:end -->

Every command has `--help`, examples and `--json`. Writes are previews until `--write` (`new` and
`set` write directly and accept `--dry-run`).

## Using with AI agents

`spec agents --for <claude|codex|gemini|cursor|copilot|generic>` prints instruction lines,
`spec mcp` serves every command as an MCP tool over stdio, and `[[assets]]` in `spec.toml`
declares skills, rules and subagents that `spec check` keeps consistent. See
[docs/guides/agents.md](docs/guides/agents.md); [AGENTS.md](AGENTS.md) is for work *on* specfold.

## Guides

- [Concepts](docs/concepts/index.md): types, ids, statuses, relations, times, task items
- [Editing documents](docs/guides/editing.md): `spec new`, `set`, `edit`, `mv` recipes; [importing an existing layout](docs/guides/import.md)
- [Checks, guards, plugins and audits](docs/concepts/checks.md); [exporting to Markdown](docs/guides/export.md)
- [Attachments](docs/guides/attachments.md): files beside a document and external stores
- [Agent assets](docs/guides/agents.md): skills, rules, subagents and instruction files
- [All commands](docs/reference/commands.md)

## Exit codes and JSON

Exit codes: `0` nothing to report, `1` findings or an ambiguous answer, `2` usage error,
`3` environment error, `4` a write flag was given but a safeguard refused it (nothing written). With `--json` every command prints one object with `schema` (`spec.<command>.v1`), `ok`,
`exit`, `items` and `next`. `spec schema output` lists the schema names.

## Contributing

Issues and pull requests are welcome; read [AGENTS.md](AGENTS.md) first. Before a commit run
`cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (it checks this README).

## License

MIT or Apache-2.0, at your option.
