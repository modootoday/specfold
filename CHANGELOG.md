# Changelog

Release notes for each version are also on
[GitHub releases](https://github.com/modootoday/specfold/releases).

## Unreleased

### Behavior changes

What an existing command does differs from 0.3.0; check scripts and CI that call these.

`spec check --write-baseline` with `--rule` or `--skip` is refused with exit 2: a baseline is recorded from a full run only (C181).

Two or more `[[guards]]` with the same `baseline` file are refused when the config loads (exit 2, naming both guards): each guard's `--write-baseline` used to overwrite the other's entries (C184).

A `--write` (fix, edit, set and the rest) over a tracked file that already has uncommitted edits first copies it to `.specfold/trash/<timestamp>/`, and its `undo:` copies that back instead of `git stash`, which also took the user's own edits; the preview names such files. Files without uncommitted edits keep the `git stash` undo (C183).

`-C <dir>` now resolves relative path arguments against `<dir>`, and `next` commands include `-C <dir>`.

Write previews show an update's changed lines as unified-diff hunks (`@@ -old,n +new,m @@`): changes with an unchanged line between them, such as the refreshed `updated:` line and an edited section, are separate hunks instead of one block. In `--json`, update items carry `hunks` (each with `line`, `old_line`, `before`, `after`) in place of the top-level `before` and `after`; `line` is still the first changed line.
`spec sync --prune` previews name the trash folder `.specfold/trash/<timestamp>/` instead of the current second, so a preview and the refusal of the same write agree; the write itself uses the second it runs in and prints the real path. The pruned-files note names `.specfold/trash/`.
`spec check --rule`/`--skip` refuse (rc 2) a rule id that does not exist, as `--fail-on` does; a lower-case id is pointed at its upper-case form (C132).
`spec fix --pick` refuses (rc 2) an id that is not one of the finding's choices, listing them (C146).
`spec log --since` must be a real `YYYY-MM-DD` date, and `spec log --type`, `spec find --type` and `spec find --status` must name a known type or status; others are refused with rc 2 (C148, C153).
`--limit 0` is refused (rc 2) by every command that takes `--limit`; it showed nothing and its `--offset` hint pointed at itself (C152).
`spec new --governs` and `spec context` refuse (rc 2) an empty path or one outside the repository, instead of recording it or suggesting `spec new --governs` for it (C130).
A guard whose `range` is not `staged`, `push`, `all` or a git range (`a..b`) is refused by every command that loads `spec.toml`, naming the guard; `spec doctor` reports it as a `config` finding (C141).
- An id held by two files (I1, e.g. a `.spec/` and a `.spec.local/` copy) is refused (exit 2) by every command that takes a document, listing both paths; name one by its path. `spec doctor`'s fix names the path (C139).
- `spec edit --section`/`--new-section` refuses (exit 2) text holding a heading at the section's level or above, which would start or duplicate a section; deeper headings and fenced code are fine (C125).
- `spec import --apply --write` refuses (exit 4) while rows marked `!` still need a decision, lists them and suggests `--force`; the new `--force` imports anyway. Previews are unchanged (C155).
- `spec attach --into` refuses (exit 2) an absolute folder or one with `..`; it used to write outside the bundle (C142).
- `spec new skill|rule|agent` refuses (exit 2) a title with no ASCII words unless `--slug` is given, and a `--slug` that is not kebab-case (C147).
- `spec init` refuses (exit 2) an `--owner-kind` with an empty pattern or template, a `{n}` beyond the pattern's `*`s, or a suffix other than `:manifest`, and a `--root` that is not an owner id (C140).
- `spec set <id> title=` refuses (exit 2) emptying the required title (C128).
- `spec export … --dry-run` previews and writes nothing (it exported before): it lists `files` and, over an earlier export, `replaces` with exit 0; a plugin exporter is not called. `spec check -q` now shows config warnings too (C114, C115, C116).
- `spec set <plan> status=archived --write` refuses with exit 4 while the plan has open tasks and lists them; `--force` archives anyway, and `--dry-run` reports them with exit 0.
- Exit code `4` (CLI-17): a write flag was given but a safeguard refused it, so nothing was
  written (`--write` over `[write] max_files`, `sync --prune` without `--yes`, a hand-edited
  copy without `--force`, `export` over an earlier export or `attach` to the store without
  `--yes`). These exited `0` before.
- `spec set <id> key=[a, b]` stores a list, not the string `"[a, b]"`.
- `spec sync --prune --write --yes` no longer deletes: each pruned file moves to `.specfold/trash/<stamp>/` (ignored by git), and the undo line copies it back (`cp -Rp .specfold/trash/<stamp>/. .`), so a file never committed can be restored. The undo of a sync or copy lists only the files written, not their sources; a successful sync ends with `next: git status --short` (C100, C101, C106).
- `spec check --write-baseline` without `[check] baseline` says what to add to spec.toml before previewing, with `next: spec explain config` (C102). `--dry-run` is accepted on every command guarded by `--yes` or `--write` and writes nothing: check and guard ignore it, import previews even with `--apply` (C120), and export previews instead (C103, C114). An export whose `--out` is outside the repository suggests `ls <out>` instead of `git status` (C104).
- `spec new` warns about the 0.4 `--write` change only when it wrote something; an invalid spec.toml points to `spec explain config` (C106).
- Overwrites need ownership or `--force` (CLI-4): `spec sync assets --write` overwrites a copy file only when its content is a version its source has had (git history or now); a hand-edited copy needs `--force`. `spec fetch` leaves a different existing file alone and fetches nothing unless `--force`. `init` upgrades and `export` are unchanged.
- Bulk writes are bounded (CLI-7): `[write] max_files = 50` (0 = no bound). When `spec mv`, `fix`, `sync` or `import` would change more files, the preview says how many and `--write` alone writes nothing; narrow the scope or add `--yes` (new on mv, fix and import).
- One meaning per shared flag (CLI-11): `spec guard --tree` (was `--all`), `spec find --body` (was `--text`), `spec init --owner-kind` (was `--owner`). The old names keep working as hidden aliases with a stderr warning; help JSON lists them under each flag's `deprecated_aliases`.
- `spec --help` and `spec help` list the 12 core commands (context, find, show, graph, new, set, edit, check, guard, fix, explain, init); `spec --help --all` / `spec help --all` list every command. `spec help --json`, `--markdown`, `--llms` and MCP still describe every command; JSON adds `core`.
- `spec set` without `--write` still writes in 0.3 but warns on stderr. **From 0.4, `set` (like `new`) previews unless `--write`.**
- `spec guard --write-baseline` without `--yes` previews the hit counts it would record.
- CLI conventions CLI-1–CLI-16 (AGENTS.md, `tests/cli_contract.rs`). Actions that delete,
  overwrite or send files out now need `--yes` besides their write flag; without it they write
  nothing and `next` shows the command with `--yes`: `spec sync assets --prune --write`,
  `spec attach … --write` when a file goes to the store, `spec export markdown` replacing an
  earlier export in `--out`, and `spec check`/`spec guard --write-baseline`.
- `spec new` without `--write` or `--dry-run` still writes but warns on stderr. **From 0.4,
  `spec new` previews unless `--write`.** Add `--write` to scripts now.

### Added

- `spec audit --kind coverage`: with `[audit] coverage_types = ["sot"]`, lists each `{#ANCHOR}` heading of a current document of those types that no other document cites as `[[<id>#ANCHOR]]`. Off by default.
- Import keeps who decided: MADR `decision-makers`/`deciders`, `consulted`, `informed` and MADR 2/Log4brains `* Deciders:` body lines become `decision_makers`, `consulted` and `informed` lists, now declared `adr` fields (no S1 warning).
- `[check] link_ignore`: globs of link targets rule L8 skips.
- A config warning when `require_when` names a status the type does not have.
- A config warning when `[audit] coverage_types` names a type that does not exist.
- Rules L8 and L9 (warn). L8 checks plain Markdown links and images offline: a relative path
  must exist and a `#fragment` must match a heading's GitHub slug (repeats get `-1`, `-2`) or a
  `{#anchor}` in the target file; URLs and ids are skipped. L9 flags an image without alt text.
- New rule S8 (warning): a field that `require_when` requires for the current status must hold a usable value — not a placeholder (`[types.<t>] placeholders`, default `TBD`, `Pending`, `TODO`, `?`), of the kind declared in `fields = { approvers = "people", resolution = "url", review_by = "date", decision = "link" }`, and not shared within a `distinct = [["authors", "approvers"]]` group. `fields` as a list works as before.
- `spec help --json` marks each command's `deprecated_aliases` (`name`, `use`, `since`). A deprecated name keeps running the new command and warns on stderr (none yet).
- `spec set ID key=` with an empty value deletes the key (an emptied `rel` is removed too).
- D1 also counts staged deletes and the old paths of renames (`p (removed)`). Its fix is a runnable `spec new change ... --depends-on ID --write`; `spec explain D1` points to `spec check --staged --rule D1`. A staged/changed run without documents says so in its summary.
- `spec new --depends-on ID` (repeatable) writes `rel.depends_on`.
- `spec export` reports attachments it did not scan for secrets (`not_scanned`); binary files no longer get the `scan_max_bytes` hint. Success `next` is `git status --short -- <out>`.
- `--dry-run` on every write command (`edit`, `mv`, `fix`, `init`, `attach`, `sync`, `audit`;
  `import --plan` takes it as an alias; `new` and `set` keep theirs).
- `spec help --json` gives each command a `risk`: read, write, destructive or external. MCP tools
  carry the matching `annotations` (readOnlyHint, destructiveHint, idempotentHint, openWorldHint).
- Writes report an `undo` command (`git stash push --include-untracked -- <paths>`).
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
  Attachments it skips are named in one U2 warning (and a `not_scanned` line in
  `spec guard secrets`) that points at raising `[files] scan_max_bytes`.
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
- C222: `spec export markdown` in a repository with no documents creates the folder with the manifest alone (rc 0, next `spec new`) instead of a raw os error
- C221: `spec export markdown --out` resolves the nearest existing folder through symbolic links, so a link into `.spec`, `.spec.local` or `.git` (or one containing the repository) is refused like the plain path
- C216: `spec guard <unknown>` lists `secrets (built-in)` among the guards it has, with next `spec guard secrets`
- C214: `spec fix` resolves its targets like `spec check` (a missing file, a directory, a non-document or `:abc` is rc 2; ids are accepted)
- C204: T1 on a draft names the two-step fix (`status=active --write && status=superseded`), and T1 and `spec set status=superseded` ignore a cross-type `rel.supersedes` that L3 rejects
- C213: `spec graph chain` walks a supersede cycle once, reports `current: null` ("a supersede cycle, nothing is current") and points at `spec check --rule T1`
- C212: `spec census --translations` rows are paged items (--jsonl, --limit, --offset), and `spec graph --jsonl` streams one row per node, edge and governed path
- C211: `spec import` walks sources in path order, so records sharing a date get ids in file-name order on every filesystem
- C210: `spec init --owner-kind` and spec.toml refuse an owner pattern segment that mixes glob characters (`**`, `svc-*`) with rc 2, and {N} counts whole `*` segments
- C207: `context` and `graph governs` on a folder also list documents whose governs path or glob lies inside it, and no longer suggest `spec new sot` then
- C206: concurrent `spec new` runs reserve their timestamp id with create-new and take the next second on a clash; I1's explain gives a repair path for timestamp ids
- C201: inline code spans close only at a backtick run of the same length (CommonMark), so a link after ``a ` b`` is checked and counted
- C200: S1 reports a title that is not a non-empty string and S4 a status that is not a string; `spec set title=[x, y]` writes quoted text, not a list
- C199: set, edit and mv keep a document's leading UTF-8 BOM; the preview no longer shows line 1 changing
- C198: a section name picks an exact heading or {#anchor} before a heading that only starts with it; several prefix matches or duplicate headings are rc 2 listing them
- C215: an argument that is not UTF-8 is rc 2 with the argument named, instead of a panic
- C218: `spec help --markdown`/`--llms` put `--flag` words in prose in code spans, so the docs site no longer renders `--write` as an en dash
- `spec guard --range <rev>` without `..` is refused rc 2 (next: `--range <rev>..HEAD`), as the config already did; it used to scan the history of `<rev>` and pass the new commits.
- `spec edit --text`/`--file` refuses text that opens a code fence and never closes it (rc 2), which used to turn every later heading into code and hide sections and open tasks.
- Undo lines work for git-ignored paths (the local tier: `rm -f` instead of a stash git cannot reach) and for documents never committed (`set`/`edit` copy the old file to the trash instead of a stash that removes the whole document).
- `spec attach` no longer silently overwrites a bundle file with other content (repo or store): the preview marks it `replace`, the write needs `--yes` (and `--force` for a file the document does not list, as `spec fetch`), and the old file is copied to the trash with an `undo:` line.
- `spec mcp` refuses a tool argument whose type does not match the schema (a string for a boolean, a non-integer limit) and a positional value starting with `-`, so a value can no longer become a flag such as `--write` or `--yes`.
- `spec audit record` and `spec audit --record` accept `--slug` (validated like `spec new`), so a title with no ASCII words can be recorded; `--write` now prints an `undo:` line.
- A guard baseline that passes through a symbolic link into a folder that does not exist yet and back out (`lnk/sub/../.guard-base.json`) is compared where the write would land: the deepest existing part of the path is resolved before `..` is applied. Such baselines are refused as shared instead of overwriting another guard's baseline and creating `sub/` (C195).
- Two guards whose baselines reach one file through a symbolic link folder (`lnk/.guard-base.json` and `./.guard-base.json` with `lnk -> .`) are refused as sharing it: an existing folder is resolved before the paths are compared (C194).
- `spec graph --format dot` names nodes `d0`/`p0` with the text as `label`, like mermaid, so a governs path spelled like a document id no longer redeclares that document as a folder or lands its edge on it. Scripts that matched quoted id node names must match labels now (C190).
- `spec init --upgrade` reads a spec.toml that starts with a UTF-8 BOM (and keeps the BOM when it writes), so the upgrade `check` asks for no longer reports "is current" and loops (C191).
- `spec edit --new-section` refuses (exit 2) a name that is empty or blank, holds a line break, or starts with `#`, as `new --title` does, instead of writing an empty, doubled or `## ##` heading (C188).
- `-C <file>` is refused (exit 3) instead of running against no repository, and an empty `-C ` value points at the help of the command typed rather than an unrelated one (C193).
- `-C <subfolder>` now acts like `cd`: relative path arguments are read from it and rebased to the repository root (`context`, `new --governs`, …), and undo/fix shell commands run from the root (`git -C <root>`), so the printed undo works (C192).
- `spec export --local --no-local` is refused (exit 2) in either order instead of silently dropping the local documents the preview counted (C189).
- `spec context` and `spec check` hint `--governs` with glob characters escaped (`src/[[]a]/z.rs`), and `spec new --governs` escapes an existing literal path holding `[ * ? {`, so the document governs that file and not the ones a glob reading would match (C185).
- The shared-baseline check resolves `.` and `..` and paths under the repository root before comparing, so `x/../b.json` and `./b.json` count as one file (C186).
- Two writes in the same second no longer share a `.specfold/trash/<timestamp>/` folder: each write claims a fresh one (`<timestamp>-1`, `-2`, … when taken), so an earlier trash copy is never overwritten. This applies to edit, fix, set, mv, import and sync --prune (C187).
- `spec edit --row` with a key that several rows share now says so, lists their line numbers and points at editing the whole section; it asks for --section only when the rows sit in different sections (C182).
- `spec mv` rewrites reference definitions, angle-bracket and percent-encoded relative links too, and no longer touches links inside code (C180).
- `spec import --apply --write` now rewrites reference definitions, angle-bracket links and percent-encoded links (keeping the encoding), still skipping code; L8 also checks reference definitions (C180).

- `spec show <id> --body` cut by `--max-bytes` now continues with `spec show <id> --section …` instead of putting the body text where the id goes (C179).
- `spec new` with an option it does not have (`--tier local`, `--bogus x`) now points at `spec help new` instead of a next that repeats the same error (C178).
- `spec export markdown --dry-run` (and the replace refusal) now print a next that keeps `--banner`, `--local` and `--fail-on`, so following it writes what was previewed; `--no-guard` is still left for you to type (C177).
- Next steps and fix commands shell-quote every path and id through one function, so `spec context` and `audit` gap suggestions run as printed for paths with spaces, quotes, or non-ASCII names.
- Audit: `--kind stale` shell-quotes governed paths in its `git diff` next step.
- Audit: `--kind stale` suggests only `spec set <id> verified_at=HEAD` when `verified_at` is not a commit, instead of a `git diff` that cannot run.
- Help: `show` and `edit` examples name sections new documents have (`rule`, `Steps`).
- `spec context`: the next names `--section rule` only for a sot; other types get `spec show <id>`.
- `spec check --format sarif`: `artifactLocation.uri` percent-encodes each path segment (a space is `%20`).
- `spec init --upgrade`: a spec.toml from a newer version is refused with the same message and exit code as `spec check`, not reported as current.
- `spec init --upgrade`: a table header with a trailing comment (`[spec] # main`) is read as TOML reads it, so an old schema under it is raised as `spec check` asks.
- `spec fix --write` with nothing to fix says "no change" and reports `written: false`, with no empty undo line (as `mv` and `set`).
- `spec fix --rule`: an unknown or lower-case rule is refused (exit 2, "no such rule", upper-case hint) as in `spec check --rule`.
- `spec export`: a refused run's next keeps `--dry-run`, `--local`, `--banner` and `--fail-on`, so following it from a preview no longer writes.
- `spec new`: the draft-supersede refusal now names `spec new adr --title '…' --from <id>`, and the missing-`--title` next keeps `--from` and the other options given.
- C164: `spec mv` to the document's own id and path reports "no change" with `written: false` and no undo, as `set` does, instead of `0 change(s) written`.
- C163: with `-C <dir>`, shell commands in `next:` and `undo:` run from where you typed them: git ones get `git -C <dir>`, others `cd <dir> &&`; check `fix:` hints get `spec -C <dir>`. Output without `-C` is unchanged.
- C162: `new --title` and `set title=` also refuse U+0085, U+2028 and U+2029 (YAML 1.1 line breaks), as for `\n` and `\r`.
- C161: with `-C <dir>`, relative path arguments (`attach` files, `edit --file`, `import --map`, `export --out`, `audit --from`) are read from `<dir>` instead of the shell's directory, and `next:` commands keep `-C <dir>`.
- `spec new --title` and `spec set title=` refuse a title with a line break (exit 2, with a next
  command to give it on one line); it split the body heading and one-row-per-document lists.
- `spec export markdown` looks up relation targets by id instead of scanning every document for
  each one: 10,000 documents export in about 1 s instead of 4.8 s, with the same files.
A write preview of a very large change no longer runs out of memory: hunks are computed only while the compared region stays under about four million line pairs, and a larger change is shown as one hunk; 30,000 replaced lines had aborted (C158).
Preview hunk headers match `git diff -U0`: an empty side names the line before the change, a count of 1 is left out, and an insertion or deletion among repeated lines sits where git puts it; JSON `line` and `old_line` follow the same rule (C159).
The pre-push snippet from `spec hook print` no longer fails on a branch without an upstream: it compares with the merge-base of the default branch (`origin/HEAD`), and when there is neither it prints a note and skips (C126).
The `spec edit` preview diff on a CRLF document compares lines with their original endings, so it shows only the changed lines instead of the whole document (C157).
A refused `--slug` (e.g. `spec new skill --slug 'Bad Slug'`) is replaced in the suggested rerun instead of given twice (C156).
A failing `spec verify` suggests `spec show` with the failing document's id instead of a literal `<id>` (C129).
`spec log` suggests `spec show` with a real id, and no next when nothing was listed (C149).
`spec graph --format box` writes a cycle note on one line with ids only (C131).
`spec check --write-baseline` without `[check] baseline` now carries the reason in JSON `error` (C134).
LSP diagnostic messages keep a real line break before `fix:` instead of `\u{000A}` (C144).
The `spec fix` preview's next keeps `--staged`, `--pick` and the targets, so following it writes what was previewed (C145).
The usage-error next of `spec new` no longer takes option values as type or title, and keeps `--title`/`--governs` (C133).
A plugin that writes a lot to stderr no longer stalls until its timeout; stdout and stderr are read together and the error shows the last stderr line (C150).
A plugin that leaves a background child holding its pipes now stops at its `timeout_ms`; `spec plugins` reports a bad action before starting any plugin (C151).
- Writes keep a file's CRLF line endings instead of mixing in LF (C154).
- `spec mv` rewrites relative Markdown links to the moved file (fragment kept), and its next includes L8 (C137).
- `spec audit --record` cites findings as `[[id]]` so the record passes L2, and audit records are no longer reported as orphans (C136).
- `spec attach` previews and refusals keep `--into` in their next (C143).
- Code fences close only with the same character and at least the same length, so a ```` fence holding ``` lines or a `~~~` fence is read alike by every command (C135).
- `spec import` counts the alias table toward `[write] max_files` (C138).
- `spec fetch` downloads to a temporary file and replaces the target only after the checksum matches; on a mismatch it removes the temporary file, leaves the target alone and exits 3 (C127).
- `spec sync assets`: a path that is a folder on one side and a file on the other is named in the preview and the write alike (write refused, exit 4); `--prune` moves the copy's entry to the trash first. Folder errors name the path (C124).
- `spec import --apply --write`'s undo brings back source documents that were never committed: they are copied to `.specfold/trash/<stamp>/` before the write and copied back by the undo, alongside the `git stash` for committed sources (C123).
- `spec import --apply --dry-run` previews (rc 0, nothing written) instead of failing with a clash against `--plan`; `--dry-run` is listed in `spec help import`, and still refuses `--yes` (C120).
- `spec import --apply --write` lists the files it created, moved and updated (alias table included) and prints `undo:`; JSON gains `written`, `undo` and `changes`. With no source documents it writes nothing, not even the alias table (C121).
- `spec mv --write` on a document not yet committed offers an undo that works: a copy of the uncommitted file is kept under `.specfold/trash/` and the undo removes the new path and copies it back, instead of a `git stash` that failed on the old path and left a stray stash (C122).
- `spec set` with values the document already has writes nothing: it reports `no change`, exits 0, and neither bumps `updated` nor offers an undo.
- `spec import` plans note body lines (`* Status:`, `* Deciders:`, `* Date:` …) that repeat values moved into frontmatter, and an H1 that keeps a number the title dropped.
- `spec audit` suggests checking open tasks (or `--force`) before archiving an active plan that still has them; `spec check`'s summary counts spec.toml warnings (`+N in spec.toml`, field `config_warnings`); a mistyped `[audit] coverage_types` entry gets a did-you-mean, and `spec doctor` points at the type name to fix.
- `spec mv` previews keep `--owner`, `--slug`, `--tier` and `--cited-paths` in the next command.
- After `spec set` changes a status or a required field, the next check includes S6 and S8.
- Nexts add `--force` (sync over a hand-edited copy) and `--yes` (fix over `max_files`); the S6
  fix names every missing field.
- Docs say `--owner-kind` where they still said `--owner`.
- An attachment that changed because a secret was added to it lists U2 before E1, and E1
  points at `spec explain U2` instead of suggesting to attach the leaky file again. Secret
  findings now come first among findings of their level.
- `spec help <command>` gives the first example as `next` without its column padding and
  `# ...` comment (`spec help guard` ended with the padded example line).
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
