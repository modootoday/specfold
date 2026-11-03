---
title: Configuration
description: The spec.toml tables and what each one controls.
---

# Configuration

> [!TIP]
> `spec init` writes a starter `spec.toml`. `spec explain config` prints this list from the CLI
> itself.

## Layout and owners

| table | controls |
| --- | --- |
| `[spec]` | format version, the document directory name (`.spec`), and `submodule_remotes` |
| `[owners]` | the root owner, owner patterns, manifests, retired directories and `nested` independent projects |
| `[public]` | published paths that hold no documents, and where their documents go |
| `[local]` | the git-ignored directory for drafts and private notes |
| `[external.<name>]` | directories outside the repository whose citations are checked |
| `[remotes.<name>]` | other repositories on disk whose documents are named `<name>:<id>`, read-only |

`[spec] submodule_remotes = true` registers each `.gitmodules` submodule that has its own
`spec.toml` as a read-only remote named after its last path segment (`vendor/child` → `child:<id>`).
An explicit `[remotes.<name>]` of the same name wins. `spec context` on a path inside such a
submodule shows that project's owner and documents; write them from inside the submodule. Off by
default, so nothing changes until it is set.

`[public] submodules_manifest = "<file>.json"` names a manifest of which submodules are public;
V1 then treats each public one as a published path. The manifest records the SHA-256 of
`.gitmodules` it was measured against, and V1 reports it when `.gitmodules` has changed since.
`spec sync visibility` measures it: it reads every submodule's URL from `.gitmodules`, asks GitHub
through the `gh` client whether each repository is public, and previews the manifest;
`spec sync visibility --write` writes it. A run that finds nothing new leaves the file as it is,
`measuredAt` included. A submodule URL off github.com, or no way to reach GitHub, stops the run
and writes nothing.

```json
{
  "gitmodulesSha256": "<sha-256 of .gitmodules>",
  "measuredAt": "2026-01-02T03:04:05Z",
  "submodules": {
    "services/blog": { "url": "https://github.com/acme/blog.git", "visibility": "PUBLIC" }
  }
}
```

### A private root for a public repository

A repository with no spec.toml can keep its documents in a private directory outside its tree.
List it in the user-level file `$SPECFOLD_ROOTS`, else `$XDG_CONFIG_HOME/specfold/roots.toml`, else
`~/.config/specfold/roots.toml`:

```toml
[[root]]
repo = "~/src/public-lib"
spec = "~/notes/public-lib-spec"   # holds its own spec.toml (spec -C <dir> init --write)

[prefix]
lib = "~/src/other-lib"   # spec show lib:<id>, read-only
```

`spec show <id> --root <repo>` (or `--in`) reads another repository explicitly and wins; otherwise
`<prefix>:<id>` is looked up in `[remotes]`, then in `[prefix]` here. An unknown prefix is an
error naming it; no path is guessed. A mapped repository whose private directory lacks spec.toml
is an error too, never a fallback to a `.spec` in the public tree.

Run inside the public repository, every command reads and writes the private directory, so ids
resolve there and nothing is written in the public tree. A spec.toml in the repository itself
always wins; without a mapping nothing changes.

When the private directory publishes the repository as one of its `[public] paths`, a path typed
inside the public repository (`spec new --governs src/a.rs`, `spec context`, `spec graph governs`)
is read below that public path, so `src/a.rs` is `packages/cli/src/a.rs`. The public path is the
entry's `public` key, else the only one listed, else the one whose last part is the repository's
directory name. When none applies, a path that does not exist in the private directory is refused:

```toml
[[root]]
repo = "~/src/cli"
spec = "~/src/workspace"
public = "packages/cli"   # needed only when [public] lists several and none is named like the repository
```

## Documents

| table | controls |
| --- | --- |
| `[types.<name>]` | override, disable or add a type: key, statuses, fields, transitions, required sections, `current`, `dir` |
| `[types.<name>]` values (S8) | `fields = { approvers = "people", resolution = "url", review_by = "date", decision = "link" }` adds the keys and their kinds (a list still replaces the built-in keys); `placeholders = [...]` (default `TBD`, `Pending`, `TODO`, `?`); `distinct = [["authors", "approvers"]]`. Checked only where `require_when` requires the field |
| `[types.<name>] key` | `"stamp"` (a UTC timestamp, the default for new types), `"slug"`, or `"number"`: files are `NNNN-<slug>.<type>.md` and ids `<type>:<owner>/NNNN` |
| `[types.<name>] current` | the statuses `spec find`, `spec context` and `spec audit` show without `--all`; by default `draft` and `active` |
| `[types.<name>] dir` | a directory of the type's own, relative to each owner's root, instead of `<[spec] dir>/<type>` |
| `[forbid]` | retired layouts and identifiers (rule Z1) |
| `[aliases]` | the frozen alias table written by `spec import` |
| `[files]` | `inline_max_bytes`, `scan_max_bytes` (repository text attachments up to this size are read by rule U2; 1 MiB by default), and `[files.store]`: `kind = "dir"` with `path`, or `kind = "command"` with `put`/`get`/`exists` argv templates (see the example) |
| `[[assets]]` | agent skills, rules, subagents and instruction files |

### A type in a directory of its own

By default every document lives in `<owner>/<[spec] dir>/<type>/`. A type with `dir` keeps its
documents in that directory instead, relative to the owner's root: the directory holding spec.toml
for the root owner, the owner's directory for every other owner. A repository that publishes its
decisions and proposals at the top, as many open specifications do, can write:

```toml
[types.adr]
dir = "adr"              # adr/20260101000000-use-files.adr.md, services/shop/adr/… for the shop

[types.proposal]
about = "An enhancement proposal."
key = "number"           # proposals/0001-scan-cache.proposal.md, id proposal:<owner>/0001
dir = "proposals"
```

Documents there are like any other: the same file names and bundles, ids, citations, `spec new`,
`spec mv`, every check, the census, translations, the language server and export. Only the
committed tier moves; the local tier keeps `<[local] path>/<owner>/<type>/`. A file in the
directory that is no document of the type is reported by S3, as in a spec directory; a nested
owner's files there stay that owner's.

`spec check` refuses a spec.toml where a `dir` is absolute or climbs out of the owner (`..`),
names the owner's root itself, overlaps `[spec] dir` or `[local] path` (lies in it or holds it), is
shared by two types, or lies inside another type's `dir`: each would give one path two readings.

Setting `dir` on a type that already has documents leaves them where they are; S3 reports each one
with the move that takes it over, `spec mv <id> --write`, which also rewrites every reference.
Dropping `dir` again needs the files moved back first, since documents outside every known
directory are no longer read.

### Numbered documents

With `key = "number"`, `spec new` takes the owner's next free number, one above the highest in use,
padded to four digits (`0001`; wider numbers stay as they are). `spec new <type> --number 42` picks
one instead, such as a pull request's number, and is refused when the number is taken. `spec mv`
keeps the number, also across owners, where it must be free.

### A proposal lifecycle

`statuses`, `transitions`, `require_when`, `fields` and `current` together describe a lifecycle such
as Draft → Accepted → Final, with Withdrawn from either open state:

```toml
[types.proposal]
statuses = ["draft", "accepted", "final", "withdrawn"]
current = ["draft", "accepted", "final"]   # spec find shows final proposals; withdrawn ones need --all
fields = { conformance = "url" }           # or "link" for a document in the repository

[types.proposal.transitions]
draft = ["accepted", "withdrawn"]
accepted = ["final", "withdrawn"]

[types.proposal.require_when]
final = ["conformance"]                    # S6: final needs it; S8: it must be an http(s) URL
```

## Checks

| table | controls |
| --- | --- |
| `[check]` | baseline (one file for every owner under this spec.toml: `spec mv` rewrites a moved document's entries in place, also when it moves to another owner; a submodule or remote with its own spec.toml keeps its own baseline, which this repository never writes), strict paths, rule severity, `extra_keys`, `count_paths`, `link_ignore`, `d1` (`"all"`, the default, or `"specific"`: rule D1 asks only the most specific document governing a changed file), `remote_status` (`true`: rule L3 warns when a relation names a `<remote>:<id>` that is superseded, archived or rejected there, with the successor to point at) |
| `[[check.override]]` | `paths` (globs), `rule`, `level` (`off`, `warn`, `error`): the rule's level for findings in those files, applied after `[check.severity]`; the last matching entry wins |
| `[[guards]]` | named checks over files and commit messages (rule U1); `public = true` reads every published path but nested projects, `nested = true` every nested project |
| `[[plugins]]` | external programs speaking `specfold.plugin.v1` |
| `[translations]` | translations paired by place: `layout`, `languages`, `source_lang`, `strict` (rule R2) |
| `[audit]` | thresholds for stale, stalled and draft documents; `coverage_types` lists types whose `{#ANCHOR}` headings must each be cited (`spec audit --kind coverage`) |
| `[verify]` | `timeout_ms` (deadline per command), `jobs`, `serial` (regexes for heavy commands run one at a time), `full_run_on` (paths whose change runs every command; a spec.toml edit that touches only `[remotes]`, `[prefix]` or `[check] baseline` does not count) and `shell` for `spec verify` |
| `[cites]` | files outside documents whose citations rule L6 checks |
| `[write]` | `max_files`: mv, fix, sync and import need `--yes` to write more files than this at once (default 50; 0 = no bound) |
| `[mcp]` | `redact`: hide secrets in what `spec mcp` returns (on by default) |

## Cache

Scan, file-hash, guard and L6 citation results are cached per repository path outside the repository: `$SPECFOLD_CACHE`, else `$XDG_CACHE_HOME/specfold`, `%LOCALAPPDATA%\specfold`, `~/Library/Caches/specfold` or `~/.cache/specfold`. Losing it only costs one slower run. The directory stays bounded on its own: at most once an hour a run removes files unused for 30 days and then the least recently used ones while it holds more than 64 MB; deleting it by hand is always safe.

To see where a slow run spends its time, set `SPECFOLD_TRACE=1`: stderr lists the scan phases, the graph, the scope, each rule (they run in parallel, so rule times overlap), baseline matching, other owners' findings and output. Within L6 it lists the file walk, how many files were read from disk, from the index (`git show`, staged runs) or reused from the cache, with bytes and read time, each remote corpus loaded and the matching time; within X1 the selection, the command run and the five slowest commands.

L6 keeps, for whole-tree runs, each cited file's references keyed by its size and modification time. A file is read again when either changes, when it was modified in the last two seconds, when an editor buffer covers it, or when a path in it may now name a document (a new document or alias); staged and ranged runs read the index or the changed lines and are not cached.

## Example

```toml
[owners]
root = "root"
# Independent projects mounted here: their own spec.toml, documents and CI. Never owners; the
# one at packages/cli is the remote cli ([[cli:adr:cli/20261030000000]]).
nested = ["packages/cli"]

[[owners.kinds]]
pattern = "services/*"
id = "{1}"
manifest = false

[check]
extra_keys = ["team", "review_*"]
count_paths = ["README.md", "docs/**/*.md"]
link_ignore = ["site/api/**"]   # rule L8 skips links to these paths
d1 = "specific"                 # D1 skips an overview when a narrower document governs the file

[[check.override]]
paths = ["services/legacy/**"]  # no L8 under the legacy tree
rule = "L8"
level = "off"

[translations]
layout = "suffix"   # guide:shop/setup-ko translates guide:shop/setup
languages = ["ko", "ja"]

[remotes.org]
path = "../shared-decisions"   # [[org:adr:root/20261030000000]] names a document there
paths = ["vendor/shared"]      # also tried, in order, when path is not on disk

[files]
inline_max_bytes = 1048576     # larger attachments go to the store

[files.store]
kind = "dir"                   # or "command" with put/get/exists, e.g.
path = "~/specfold-store"      # put = ["rclone", "copyto", "{src}", "remote:bucket/{key}"]
```

A `rel.governs` entry that belongs to another project (written `<remote>:<path>`, under a remote
checked out inside this repository, under a nested project, or under a submodule that no owner
kind claims) is external: P1 and L5 leave it out, and a document governing only such paths stays
where it is. D1 still matches changes under it. A submodule an owner kind claims (`apps/*` over
a product kept in its own repository) is that owner's code, so its documents go to its spec
directory. `spec new` starts from the owner containing the directory it runs in (`-C <dir>`).
