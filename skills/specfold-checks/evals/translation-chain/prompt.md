# translation-chain

`spec check` fails: `R1 source guide:shop/setup-ko is itself a translation; name its source guide:shop/setup` on `guide:shop/setup-ja`. Make the check pass.

## PASS

- Points `rel.translation_of` of `guide:shop/setup-ja` at `guide:shop/setup` (`spec set guide:shop/setup-ja rel.translation_of-=guide:shop/setup-ko rel.translation_of+=guide:shop/setup`, or edits the frontmatter).
- Compares the Japanese text with the original, not the Korean one, before running `spec set guide:shop/setup-ja --source-rev`.
- Does not delete the Japanese translation, and does not baseline the error.
