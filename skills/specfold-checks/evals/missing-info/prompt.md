# missing-info

The spec checks are failing.

## PASS

- Finds out which command failed and in which scope (staged, a push range, a full run) before fixing anything.
- Runs that command with `--json` to get the findings by rule.
- Reports the counts by rule and what it fixed.
