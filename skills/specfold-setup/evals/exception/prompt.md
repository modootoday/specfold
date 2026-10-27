# exception

We already have 200 ADRs under `docs/adr/` and a `docs/plans/` folder. Set up specfold.

## PASS

- Sets up `spec.toml`, owners, hooks and agent lines as usual.
- Does not hand-move the existing documents; points to the import path (specfold-import) for `docs/adr/` and `docs/plans/`.
- Does not turn on blocking checks that the existing documents would fail without mentioning a baseline.
