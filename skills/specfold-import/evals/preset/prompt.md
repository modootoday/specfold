# preset

Our decisions are MADR 4 files in `docs/decisions/`; one says `status: "superseded by [ADR-0002](0002-use-a-queue.md)"`. Bring them into specfold. `spec.toml` has no `[aliases]` table.

## PASS

- Starts from `spec import --preset madr > import.toml` instead of writing the map from scratch.
- Adds `[aliases] file = ".spec/_aliases.tsv"` to `spec.toml` before applying.
- Plans, settles every `!` row (adding the `[status]` line the plan names), and checks that ADR-0002 gets `rel.supersedes` from the plan's notes.
- Applies with `--apply --write` only once nothing is unresolved, then runs `spec check`.
