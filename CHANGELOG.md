# Changelog

Release notes for each version are also on
[GitHub releases](https://github.com/modootoday/specfold/releases).

## Unreleased

### Added

- `spec export markdown --target obsidian|mkdocs|docusaurus|github-wiki --out <dir>`: a
  built-in exporter that writes plain Markdown with each target's links, flattened
  frontmatter, a `.specfold-export.json` manifest, and a report of references written as text
  (`--fail-on unresolved` to fail instead). `--local` adds `.spec.local` documents; `--banner`
  names each page's source. The name `markdown` is reserved for it.
- Exporter plugins can declare `wants: ["body", "links"]` to receive each document's body,
  `source_rev` hash and resolved references with byte spans; past `--max-bytes`, bodies come as
  `body_path`.
- `spec graph consumers` prints a header, a count and scope footer, and `searched` in JSON.
- `spec help lsp` takes `--dir` and points to the editor setup guide.
- `spec new` warns when the slug leaves out words in another script ("결제 비교 Gateway" gives
  `gateway`), not only when most of the title is lost.

### Changed

- `spec new` and `spec set` accept `--write` (they always write).
- `spec new --translation` no longer suggests `--title` when the title comes from the source.
- An error that ends with a runnable `spec …` command offers it as `next`.
- Exporter plugins no longer receive `.spec.local` documents unless `--local` is given; edges to
  them are left out and references to them arrive unresolved (`reason: "not exported"`).
- `spec edit --text`, `--new-section`, `--row` and `spec new --title` take values that start with
  `-`, such as `--text "- [ ] wire gateway"`.
- `spec edit --text -` is refused and points to `--file -` instead of writing a lone `-`.
- A file that is not UTF-8 is reported by `check` (S1), `fix` and `show` with its path and line,
  instead of "no frontmatter block".
- After `spec set <id> aliases=…`, the suggested check includes I2 (duplicate aliases).
- `spec edit --section` on a missing section offers the `--new-section` command as `next`;
  `spec graph --format json` says JSON output is `--json` and offers that command.

## 0.3.0

### Added

**Editors and automation**
- `spec lsp`: a language server over stdio. It reports check findings as diagnostics, checks
  unsaved buffers, offers `spec fix` replacements and each candidate (L1, L2) as quick fixes,
  runs only the `--write` fix commands it offered, and reuses the last check while nothing
  changed. Setup for Neovim, Helix and VS Code is in the editing guide.
- A composite GitHub Action that installs a verified release and runs `spec check`, with a
  merge-base comparison and `--fail-on`.
- `--max-bytes` output budget for lists and MCP (MCP default 40000).
- `cargo binstall specfold` downloads the release archive instead of compiling.

**Checking**
- `spec check` formats `github`, `sarif`, `gitlab`, `rdjson` and `editor`; `--summary`, `-q`,
  `--new` and `--fail-on`.
- `spec check <FILE|ID>...` reports only the named files or documents; `--stdin-path` checks an
  unsaved buffer, including a new file.
- New rules:
  - A1–A3: agent assets (skills, rules, subagents, instruction files).
  - A4: machine-local absolute paths (`/Users/`, `/home/`, `C:\Users\`) in agent assets.
  - H1: invisible characters (zero-width, word joiner, mid-file BOM, bidi controls) in documents
    and well-known agent files; `spec fix --rule H1` removes them.
  - N1: `spec:count` markers that disagree with the corpus; `spec fix` updates them.
  - S7: required sections per type (`[types.<t>] sections`).
  - R1: a translation whose source changed since its `source_rev`, naming the changed sections.
  - R2: translation layout (`[translations]` suffix or dir).
  - L7: pinned remote documents (`pins`).
  - T5: exceptions past their `review_by` date.
- S1 names the nearest known key for an unknown one; S4 suggests the nearest value.

**Translations**
- `rel.translation_of`, `lang` and `source_rev` track a translation against its source;
  `spec set <id> --source-rev` records the reviewed source.
- `spec new --translation <lang> --from <id>` copies a local source and records the pair.
- `spec fix --translations --write` writes down pairs found by the layout.
- `spec census --translations` reports per-language status, with `--fail-on any|stale|none`.

**Remotes and exceptions**
- `[remotes.<name>]` reads documents from another repository's checkout as `<name>:<id>`;
  L1, L3 and L6 check links to them, `spec show` and `spec resolve` read them, and `spec doctor`
  lists each remote.
- `spec set --pin <name>:<id>` and `--unpin` record or remove the remote revision you rely on.
- `rel.exception_of` and `review_by` record an exception to a rule with a date to revisit it;
  `spec audit` lists overdue exceptions.
- `spec graph consumers` lists documents that translate or pin a given document.

**Writing and navigation**
- `spec edit`: replace or add sections (`--new-section --after`), edit table rows (`--row`,
  `--cell`, `--append`), `--replace` and stdin input.
- `spec new`: title as a positional argument, slugs for non-ASCII titles, templates for skills,
  rules and agents, refusal of forbidden paths with a `--local` hint.
- `spec doctor`, `spec init --upgrade`, `spec assets`, `spec sync`, `spec show --tasks`,
  `spec find --open-tasks`; `list` and `ls` are aliases of `spec find`.
- `spec graph`: tree, box, mermaid and dot views; impact, chain and governs.
- A built-in `change` type for user-facing change notes.
- `spec import`: both spellings of "supersedes", supersession links recorded both ways, links
  between imported records rewritten, relations recorded as full ids, and a next-steps plan.
- Documents open by id prefix and by full timestamp.

### Changed
- `created` and `updated` are written as RFC 3339 UTC; plain dates and offsets are still read.
- Status moves must name a reachable status; `spec set` no longer guesses a finishing status.
- R1 reports translation chains and self-references as errors, which can fail an existing CI job.
- `spec new --translation` refuses a source in a remote or a source that is itself a
  translation; the language tag in the name is lower case.
- A self-exception is refused, and `review_by` must be a valid date.
- Every error ends with one runnable next command.
- spec.toml typo warnings appear once, in `spec check` (not `-q`) and `spec doctor`.
- pre-commit hooks use `language: unsupported`.
- `spec --version` includes the commit hash.
- `spec doctor --format json` gains a `remotes` field.

### Fixed
- `spec mv` rewrites stale path cites matched by file name, and cites in all tracked files
  (`--cited-paths`).
- A3 no longer treats git refs such as `origin/main` as paths; A4 ignores the CI runner home
  `/home/runner/`.
- `spec lsp` answers malformed JSON with -32700 and exits with a reason on a broken frame.
- `review_by` and `pins` are no longer reported as unknown keys by S1.
- `spec fix` previews show the line before and after.
