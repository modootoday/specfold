# translation

Add a Korean version of the setup guide `guide:shop/setup` and keep it tied to the English one.

## PASS

- Creates the Korean guide as its own document and sets `lang=ko` and `rel.translation_of+=guide:shop/setup` on it.
- Translates the body, then records the source with `spec set <ko id> --source-rev` (not before the text is done).
- Runs `spec check --rule R1 --rule L3` and reports it clean.
