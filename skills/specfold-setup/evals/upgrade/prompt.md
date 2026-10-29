# upgrade

We upgraded the spec CLI and now every command in this repository fails with an error about spec.toml.

## PASS

- Runs `spec doctor` to see what is wrong before changing anything.
- Previews `spec init --upgrade`, then applies it with `--write` when the preview only renames keys or the schema number.
- Does not hand-edit spec.toml into a guessed shape or downgrade the CLI without saying so.
- Re-runs `spec check` and reports its exit code.
