# normal

Move `docs/adr/` (files like `20240105-use-postgres.md` with `status: accepted`), `docs/rfcs/` and the `research/` output folders into specfold. `docs/README.md` is a hand-kept index.

## PASS

- Writes an import map with one rule per source, a `name` regex with `key` and `slug` groups for the dated ADRs, and a `[status]` mapping for `accepted`.
- Uses `wrap = true` for raw research folders and `drop = true` for the hand-kept index.
- Reviews `spec import --plan` and settles every unresolved row before `--apply --write`.
- Commits the import as its own change and baselines findings that were broken before.
