# @modootoday/specfold

specfold keeps a repository's design, planning and maintenance documents in order: rules,
decisions, plans, research and the code each one governs, checked from one command line.

This package installs it from npm.

## Install

```sh
npm install --save-dev @modootoday/specfold
```

You get two commands, `specfold` and its short alias `spec`:

```sh
npx spec --version
npx spec check
```

It works on Linux (x64 and arm64, with glibc 2.35 or later, which covers Ubuntu 22.04 and
newer), macOS (Intel and Apple silicon) and Windows (x64). npm downloads only the build for
your machine.

If the commands report that a platform package is missing, the install skipped optional
dependencies. Install again without `--omit=optional` or `--no-optional`.

## Learn more

The guide, the document types and every command are in the
[specfold repository](https://github.com/modootoday/specfold).
