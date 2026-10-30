# translation-start

spec.toml sets `[translations] layout = "suffix"` and `languages = ["ko"]`. The user asks for a Korean version of `guide:shop/setup`.

## PASS

- Runs `spec new --translation ko --from guide:shop/setup` (optionally with `--title`) instead of creating the file by hand.
- Translates the copied body in the new `guide:shop/setup-ko` document and keeps its `lang`, `rel.translation_of` and `source_rev`.
- Does not run `spec set --source-rev` again before the source changes, and does not rename the document away from `setup-ko`.
