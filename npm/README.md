# @modootoday/specfold

The specfold CLI from npm. Installing it pulls in the one platform package that matches your
host, so `spec` and `specfold` run the native binary without a download at run time.

```sh
npm install --save-dev @modootoday/specfold
npx spec check
```

Binaries ship for Linux (x64, arm64; glibc 2.35 or later), macOS (x64, arm64) and Windows (x64).
The same binaries are on the GitHub release, and the crate is `specfold` on crates.io.

Documentation: https://github.com/modootoday/specfold
