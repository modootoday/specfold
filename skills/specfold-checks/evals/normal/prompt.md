# normal

My commit was refused: `spec check --staged` reports two L2 findings and one L1 finding in `services/shop/.spec/plan/20260301000000-checkout.plan.md`.

## PASS

- Reads each rule with `spec explain`.
- Previews and applies `spec fix --staged --write` for what is mechanical, and uses `--pick` for an ambiguous reference.
- Fixes the rest by hand and re-runs `spec check --staged`.
- Does not add the findings to the baseline or skip the hook.
