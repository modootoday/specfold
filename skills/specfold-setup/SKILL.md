---
name: specfold-setup
description: Add specfold to a repository - install the spec CLI (npm dev dependency or cargo), write spec.toml with spec init and the owner patterns that match the repository's apps and packages, wire the pre-commit and pre-push checks through the hook manager already in use, and give every coding agent the instruction lines from spec agents. Use when a repository has no spec.toml yet, when asked to set up specfold or spec checks, or to connect specfold to hooks or agents. Not for moving an existing documentation layout in (that is specfold-import) or for writing documents.
metadata:
  tier: open
  level: L2
  domain: documentation
  install: optional
  keywords: "specfold, spec.toml, spec init, owners, .spec, design documents, ADR, hooks, agent instructions"
  requires-bin: "spec"
---

# Setting up specfold in a repository

specfold keeps design, planning and maintenance documents in a `.spec/` directory beside the
code each one governs, and checks them from one CLI, `spec` (long name `specfold`). Every
command is non-interactive, previews before it writes, and ends with a suggested next command.

## 1. Install

In a JavaScript repository, pin it as a dev dependency so the lockfile fixes the version:

```sh
npm install --save-dev @modootoday/specfold
npx spec --version
```

Elsewhere, `cargo install specfold`, or a release binary from GitHub. Never run a bare
unscoped `spec` package through npx: that name is not specfold.

## 2. Find the owners

An owner is a unit that keeps its own documents: the repository root, and each app, service
or package. Look at the layout before choosing patterns:

```sh
ls -d */ */*/ | head -50
git ls-files '*package.json' '*Cargo.toml' '*pyproject.toml' '*go.mod' | head -50
```

Each `--owner` is `pattern=id-template`; `{1}` is the first wildcard. Add `:manifest` when only
directories with a package manifest count.

```sh
spec init --owner 'apps/*={1}' --owner 'packages/*=pkg.{1}:manifest'           # preview
spec init --owner 'apps/*={1}' --owner 'packages/*=pkg.{1}:manifest' --write   # write spec.toml
spec owners                                                                     # confirm
```

`spec owners` must list every unit you expect. A missing one means a pattern is wrong; fix
the pattern before anyone writes documents, because the owner decides where a document lives.

## 3. Hooks

specfold never installs hooks. Print a snippet for the hook manager the repository already
uses and add it there:

```sh
spec hook print --for husky       # or git, pre-commit, lefthook
```

The checks to run:

| When       | Command                          |
| ---------- | -------------------------------- |
| pre-commit | `spec check --staged`            |
| pre-push   | `spec check --changed <base>`    |
| pre-push   | `spec verify --changed <base>`   |

`<base>` is what the push is compared against, such as `origin/main`.

## 4. Agents

```sh
spec agents --for claude     # or codex, gemini, cursor, copilot, generic
```

Paste the printed lines into that agent's instruction file (`CLAUDE.md`, `AGENTS.md` and so
on). They tell the agent to run `spec context` before changing code and `spec check --staged`
before committing.

## 5. Large files, public paths and guards (when they apply)

- Attachments over `[files] inline_max_bytes` need `[files.store]`: a directory or a command
  the team already trusts (rclone, aws, wrangler). specfold has no network code.
- A repository that publishes some paths (a public mirror, an npm package) declares them in
  `[public] paths`, so documents are never stored under them.
- `[[guards]]` refuse denied terms, patterns or files in a range; `spec explain config` lists
  every key.

## Report

The install method and version, the owner patterns and the `spec owners` list, where the hook
lines went, which agent files got the lines, and the `spec check` exit code on the new setup.
