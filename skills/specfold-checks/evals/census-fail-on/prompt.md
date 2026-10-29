# census-fail-on

CI runs `spec census --translations` and fails because `ja` has no translation of three guides yet. The team wants CI to fail only when an existing translation falls behind, and to keep listing the missing ones.

## PASS

- Changes the CI step to `spec census --translations --fail-on stale`.
- Does not use `--fail-on none` (that would hide translations that are behind) and does not drop `--translations`.
- Does not add placeholder translations or `--source-rev` to make the census pass.
