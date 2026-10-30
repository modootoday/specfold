# translation-values

`spec check` warns on `guide:shop/setup-kr`: `R1 lang 'korean' is not a language tag (BCP 47) like ko, ja or pt-BR` and `R1 guide:shop/setup has another 'ko' translation: guide:shop/setup-ko`. Make the check pass.

## PASS

- Fixes the tag to `ko` with `spec set guide:shop/setup-kr lang=ko` (or the frontmatter); does not invent a tag such as `kr`.
- Then sees the two Korean translations and asks which one to keep, or merges them into one and removes the other with the user's agreement.
- Does not change `source_rev` by hand; records it only with `spec set <id> --source-rev`.
