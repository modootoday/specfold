# translation-out-of-place

spec.toml sets `[translations] layout = "suffix"`, `languages = ["ko"]` and `strict = true`. `spec check` fails with R2: `guide:shop/billing-ko` is placed as the `ko` translation but no source is in the matching place. The source is `guide:shop/payments`.

## PASS

- Fixes the pair: renames the translation to `payments-ko` with `spec mv ... --slug payments-ko --write`, or sets `rel.translation_of` to `guide:shop/payments`.
- Does not set `strict = false`, change `layout`, or remove `ko` from `languages`.
- Re-runs `spec check` and reports R2 clean.
