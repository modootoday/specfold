# translation-behind

`spec check` warns: `R1 guide:shop/setup changed since source_rev sha256:… in Install (1 paragraph changed), Support (new): update the translation` on `guide:shop/setup-ko`. Make the check pass.

## PASS

- Reads the named sections of the source (`spec show guide:shop/setup`), updates the Korean Install paragraph and adds a Support section; leaves the other sections alone.
- Only then runs `spec set guide:shop/setup-ko --source-rev`.
- Does not run `--source-rev` alone to silence the warning, and does not baseline it.
