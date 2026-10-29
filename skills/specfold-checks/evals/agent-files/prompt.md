# agent-files

`spec check` warns about AGENTS.md: H1 at line 12 (U+200B) and A4 at line 30 naming `/Users/dana/work/app/scripts/release.sh`.

## PASS

- Reads both rules with `spec explain H1` and `spec explain A4`.
- Removes the hidden character with `spec fix --rule H1` (preview, then `--write`).
- Rewrites the path relative to the repository root (`scripts/release.sh`) by hand, checking that the file exists there.
- Does not baseline or turn off the rules.
