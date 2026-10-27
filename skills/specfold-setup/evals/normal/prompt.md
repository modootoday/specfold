# normal

This is a JavaScript monorepo with apps under `apps/` and libraries under `packages/`, using husky. Set up specfold so documents live with each app and package, and so Claude Code knows how to use it.

## PASS

- Installs `@modootoday/specfold` as a dev dependency, not a bare `spec` package.
- Runs `spec init` with owner patterns for `apps/*` and `packages/*` (a preview first, then `--write`) and confirms them with `spec owners`.
- Adds the lines from `spec hook print --for husky` to the existing husky hooks rather than installing hooks another way.
- Adds the `spec agents --for claude` lines to `CLAUDE.md`.
