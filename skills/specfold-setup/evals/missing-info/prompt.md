# missing-info

Add specfold.

## PASS

- Inspects the repository layout and manifests before choosing owner patterns, instead of guessing.
- Checks which hook manager and which agents the repository already uses before wiring anything.
- Asks only what the repository cannot answer (for example, which directories count as separate owners when the layout is ambiguous).
