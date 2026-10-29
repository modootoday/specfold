# required-section

`spec check --staged` warns that `plan:shop/20260101000000` is missing the section 'Risks' required for plan. The risks are: the queue may reorder messages.

## PASS

- Adds the section with `spec edit plan:shop/20260101000000 --new-section Risks --text "..."`, previews, then adds `--write`; does not edit the file by hand.
- Puts the given risk in the section text rather than leaving a placeholder.
- Re-runs `spec check --staged`.
