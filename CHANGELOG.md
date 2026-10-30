# Changelog

Release notes for each version are also on
[GitHub releases](https://github.com/modootoday/specfold/releases).

## Unreleased

### Added

- Rule D1 (warning, `spec check --staged` and `--changed` only): code changed under an active
  document's `rel.governs` while neither the document nor a `change` note naming it changed.
- Rule U2 runs the built-in secrets check (the one `spec export` always applies) over every
  document file, so the commit hook (`spec check --staged`) refuses what export would refuse.
  `spec guard secrets` runs it over any range, and `spec guard` with no secrets guard configured
  includes it. A configured `secrets = true` guard keeps reporting through U1 instead.
  Hits in local (git-ignored) documents are warnings; in committed documents, errors.
  U2 also reads each document's text attachments kept in the repository (up to the new
  `[files] scan_max_bytes`, 1 MiB by default; binary files are skipped), so a secret attached
  with a warning keeps being reported by `spec check` and `spec guard secrets`. `spec export`
  reads the same attachments of the documents it exports and refuses on a hit.
  U2 also reads each document's text attachments kept in the repository (up to the new
  `[files] scan_max_bytes`, 1 MiB by default; binary files are skipped), so a secret attached
  with a warning keeps being reported by `spec check` and `spec guard secrets`. `spec export`
  reads the same attachments of the documents it exports and refuses on a hit.
- `spec attach` runs the built-in secrets check on every file, with no `[[guards]]` needed: a hit
  refuses a store upload (`--allow-hit secrets` lets it through) and warns for a repository file.
- A slug id (`plan:root/payment-rollout` for a timestamp-keyed plan) resolves in `[[…]]`
  references, Markdown links, export and `spec find`; L1 suggests the canonical id.
- `spec help <command>` shows the command's notes (limits, settings, what a refusal means)
  after its arguments and examples; `--markdown` and `--json` (`notes`) carry them too.
- The attachments guide, the config reference and `spec help attach` show a `[files.store]` example.

- Built-in secrets (`secrets = true` in a guard) now find connection strings with a password
  (`scheme://user:password@host`, any scheme: postgres, mysql, mongodb, redis, amqp …) and
  three-part JWTs. Not flagged: example and loopback hosts (`example.com|org|net`, `*.example`,
  `*.test`, `*.invalid`, `localhost`, `127.*`, `::1`, `0.0.0.0`); placeholder passwords
  (`${VAR}`, `$VAR`, `%VAR%`, `{{var}}`, `<var>`, one repeated character such as `xxxx`, words
  such as `password` or `changeme`, a password equal to the user name such as
  `postgres:postgres`, fewer than 4 characters); and the jwt.io sample token.
- `spec attach` runs every guard that reads content (secrets, denied terms or patterns, denied
  files) over files bound for the store before uploading; an error-level hit refuses, a warn
  hit is shown, and `--allow-hit <guard>` lets one guard's hits through. Matched values are
  never printed.
- `spec export` runs the built-in secrets check, and the guards marked `public = true`, over
  what leaves (the built-in tree, or the source files of the documents a plugin receives) and
  writes nothing, nor calls the plugin, on an error-level hit. The secrets check runs even when
  no guard is configured; `--no-guard` skips the check and the guards.
- Rule L1 warns about a Markdown link whose target is a document id (`[text](plan:root/…)`)
  and offers `[[id|text]]` (`spec fix --rule L1 --write`); the built-in export rewrites the
  ones that resolve into the target's link.
- A timestamp-keyed document resolves by its id with the slug in place of the timestamp:
  `plan:root/payment-rollout` names `plan:root/20261030020644`.
- `spec export markdown` ends with a next command, and its missing `--target`/`--out` and
  non-empty `--out` errors offer a runnable one. The export guide documents how same-named
  files get `-2`, `-3`.
- The Markdown source of the export guide is published at `/guides/export/index.md`.
- `spec mcp` replaces built-in secret matches with `[redacted: <pattern>]` in tool results and
  reports the count as `redacted`; `[mcp] redact = false` turns it off. `spec edit` and
  `spec set` refuse text holding a redaction marker the document does not already hold.
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

### Fixed

- `spec export` secret refusals name the source document file and line (not the exported copy)
  and suggest `spec guard secrets`.
- `spec find` with no current match suggests the same search with `--all`, runnable as written.
- `spec attach` writes `updated` as an RFC 3339 timestamp, like `spec new` and `spec set`.
- Refusing text that holds the mcp redaction marker now suggests `spec edit <id> --append`.

### Changed

- `spec new` and `spec set` accept `--write` (they always write).
- A missing-section error from `spec edit` and `spec show` no longer lists the H1 title.
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
