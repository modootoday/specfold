# specfold

One command line for a repository's design, planning and maintenance documents: rules,
decisions, plans, research, evidence, audits, page specs, design boards, guides and runbooks.

- **Documents live with the code they govern.** Each owner (the repository root, an app, a
  package) keeps its documents in its own `.spec/` directory. `spec new --governs <path>` works
  out which owner that is; you never pick a directory by hand.
- **Definitions live in one place.** Types, ids, frontmatter, statuses, relations and checks come
  from specfold and one `spec.toml`, not from conventions written in several READMEs.
- **The graph is derived, never stored.** References between documents, supersede chains and the
  code each document governs are computed on every run.
- **Built for AI agents first.** Every command is non-interactive, prints text or a stable
  `--json` schema, bounds its output, attaches a fix to every finding and ends with a suggested
  next command. The same commands are served as MCP tools by `spec mcp`.
- **Language- and agent-neutral.** Owners are found through any common package manifest; the CLI,
  JSON Schemas and MCP work with any agent that can run a command.

## Install

Pick whichever fits your project. Each gives you two commands, `specfold` and its short alias
`spec`.

With Rust installed:

```sh
cargo install specfold
```

In a JavaScript project, add it as a dev dependency so everyone on the team gets the same
version from the lockfile:

```sh
npm install --save-dev @modootoday/specfold
npx spec --version
```

Or download a ready-built binary for Linux, macOS or Windows from the
[GitHub releases](https://github.com/modootoday/specfold/releases).

## Start

```sh
spec init --owner 'services/*={1}' --owner 'libs/*=lib.{1}:manifest' --write
spec new sot --governs services/shop/src/cart --title "Cart totals"
spec context services/shop/src/cart/total.rs     # what governs this code
spec check --staged                          # before committing
spec check --new                             # list documents not in the last commit
spec check --summary                         # findings per rule; -q lists errors only
spec agents --for claude                     # lines for your agent's instructions
spec hook print --for pre-commit             # a hook for your hook manager
```

## Documents

| type     | key       | what it is                                                     |
| -------- | --------- | -------------------------------------------------------------- |
| sot      | slug      | a source of truth: the rule that holds now                     |
| adr      | timestamp | a decision and the reason for it                               |
| plan     | timestamp | a plan of work and its running record                          |
| research | timestamp | a study, benchmark or evaluation                               |
| evidence | timestamp | the cover of a bundle of measurements or raw outputs           |
| audit    | timestamp | an audit snapshot; its body is sealed once committed           |
| page     | slug      | the specification of one screen or page                        |
| design   | timestamp | a bundle of design boards                                      |
| guide    | slug      | how to develop, operate or use something                       |
| runbook  | slug      | a step-by-step operational procedure                           |

Ids are `<type>:<owner>/<key>`, computed from where a document lives. Files are
`<owner>/.spec/<type>/<key>-<slug>.<type>.md`, or a directory with `index.<type>.md` when the
document has chapters or attachments. Projects can disable, change or add types in `spec.toml`.
Timestamp keys (`YYYYMMDDHHMMSS`) and the `created` and `updated` dates are in UTC, so a
document written early in the morning east of UTC carries the previous day's date.

Statuses are `draft`, `active`, `superseded`, `archived` and `rejected`, and only move forward.
Relations are written once, on the document that makes them: `rel.governs` (code paths),
`rel.supersedes`, `rel.implements`, `rel.evidence`, `rel.depends_on`. Body references are
`[[<id>]]` or `[[<id>#<anchor>]]`. Task items (`- [ ]`, `- [x]`) are counted: `spec show` reports
a document's progress and `spec show <id> --tasks` lists each item with its file and line.

## Commands

`spec help` lists every command; `spec help --json` describes all of them with their arguments,
examples and output schemas. `spec explain` documents every rule.

| read                                | write                       | check                       |
| ----------------------------------- | --------------------------- | --------------------------- |
| context, find, show, resolve, graph | new, set, edit, mv, fix     | check, verify, guard        |
| owners, log, files                  | attach, fetch, import, init | audit, census               |

Writes are previews until `--write` (or plain for `new` and `set`, which accept `--dry-run`).
`spec edit` changes bodies: it replaces or appends to one section (`--section`) or the whole
body, from `--text`, `--file`, or stdin (`--file -`), and bumps `updated`. Sealed and closed
documents are refused.
Exit codes: 0 nothing to report, 1 findings or an ambiguous answer, 2 usage, 3 environment.

## Attachments

Non-Markdown files belong to a document and are listed, with their SHA-256, in its `files`
frontmatter by `spec attach`. Small files stay in the repository; larger ones go to a store
configured in `spec.toml`: a directory, or a command you already trust (rclone, aws, gsutil,
wrangler …). specfold contains no network code and never handles credentials.

## Guards, plugins and audits

- **Guards** (`[[guards]]`, `spec guard`) check any file and commit message in a range for denied
  terms, patterns, files, sizes and secret formats. Terms may come from a file outside the
  repository or an environment variable, and are masked in output.
- **Plugins** (`[[plugins]]`) are programs in any language that speak `specfold.plugin.v1`
  JSON over stdin and stdout to add rules, importers, exporters, audits and event handlers. Only
  programs named in `spec.toml` run.
- **Audits** (`spec audit`) find what passes the rules yet needs attention: stale documents,
  orphans, owners without a source of truth, stalled plans, open items. `--pack` groups documents
  for a reviewer, and `spec audit record` stores a review as a sealed audit document.

## Importing an existing layout

`spec import --map import.toml` reads files from any other layout through a map of patterns,
types, owners, key rules, status mappings and redirect stubs; it prints a reviewable plan and,
with `--apply --write`, moves everything and writes a frozen alias table so old ids and paths
keep resolving until they are rewritten.

## License

MIT or Apache-2.0, at your option.
