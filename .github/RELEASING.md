# Releasing

`.github/workflows/release.yml` builds the binaries into a draft release, then publishes the
crates to crates.io and the npm packages to npm, each after a reviewer approves its environment
(`crates`, `npm`), and marks the release published. By hand (workflow_dispatch) it never
publishes: mode `dry-run` builds, packs and rehearses every publish and deletes its draft; mode
`bootstrap` keeps the draft release `v<version>` with the binary archives.

1. Bump `version` in the root `Cargo.toml` (both crates share it), the `specfold-core`
   dependency version and `npm/package.json`, run `cargo test` and
   `node --test npm/release/*.test.mjs`, and commit.
2. Optionally rehearse: run the Release workflow by hand in mode `dry-run`.
3. Push the tag `v<version>`. The workflow builds the binaries into a draft release and waits
   for a reviewer to approve the `crates` and `npm` environments.
4. After both publish, it marks the release published. Confirm with
   `npm view @modootoday/specfold@<version> version` and the crates.io page.

## Packages

| Package                             | Contents                                            |
| ----------------------------------- | --------------------------------------------------- |
| `specfold`, `specfold-core` (crates) | the CLI and its library                            |
| `@modootoday/specfold`              | a launcher; names the five below as optionalDependencies |
| `@modootoday/specfold-linux-x64`    | spec, specfold (glibc 2.35 or later)                |
| `@modootoday/specfold-linux-arm64`  | spec, specfold (glibc 2.35 or later)                |
| `@modootoday/specfold-darwin-x64`   | spec, specfold                                      |
| `@modootoday/specfold-darwin-arm64` | spec, specfold                                      |
| `@modootoday/specfold-win32-x64`    | spec.exe, specfold.exe                              |

Platform packages publish first and the main package last, once npm answers for every platform
package (`npm/release/publish.sh`), so no install names a platform version that is not on npm.
A version already on npm is skipped, so a rerun is safe. A failed npm job writes which packages
reached npm to its summary (`npm/release/report.sh`).

## First npm release (bootstrap)

npm accepts a trusted publisher only for a package that already exists, so the first version of
the six npm packages is published once from a maintainer host logged in to npm:

1. Run the Release workflow by hand in mode `bootstrap` on the release commit. It leaves the
   draft release `v<version>` with one archive per target.
2. On the maintainer host, from a clean checkout of that commit:

   ```sh
   tag="v$(node -p 'require("./npm/package.json").version')"
   gh release download "$tag" --pattern 'specfold-*.tar.gz' --pattern 'specfold-*.zip' --dir bins
   for archive in bins/specfold-*; do
     case "$archive" in
       *.tar.gz) tar -C bins -xzf "$archive"; target="$(basename "$archive" .tar.gz)";;
       *.zip) unzip -q -d bins "$archive"; target="$(basename "$archive" .zip)";;
       *) continue;;
     esac
     node npm/release/platform-packages.mjs --target "${target#specfold-}" --bin-dir "bins/$target" --out platform
   done
   for dir in ./platform/*/; do npm publish "$dir" --access public; done
   node npm/release/platform-packages.mjs --optional-dependencies > optional.json
   node -e 'const fs=require("fs");const p=JSON.parse(fs.readFileSync("npm/package.json"));p.optionalDependencies=JSON.parse(fs.readFileSync("optional.json"));fs.writeFileSync("npm/package.json",JSON.stringify(p,null,2)+"\n")'
   cp LICENSE-MIT LICENSE-APACHE npm/
   npm publish ./npm --access public
   git checkout npm/package.json && rm npm/LICENSE-MIT npm/LICENSE-APACHE
   ```

3. Add the trusted publishers (below), then publish the draft release. That creates the tag,
   which runs the workflow once more: it reuses the release and its archives, publishes the
   crates after the `crates` approval, and skips every npm package already on npm.

## One-time setup

- **GitHub environments, one per registry**, each with required reviewers and the tag policy
  `v*`: `crates` and `npm`.
- **crates.io Trusted Publishing** for both crates: repository `modootoday/specfold`, workflow
  `release.yml`, environment `crates`. The very first version of each crate was published once
  by a maintainer with an API token.
- **npm Trusted Publishing** for each of the six npm packages: on npmjs.com open the package's
  Settings, Trusted Publisher, choose GitHub Actions and enter organization `modootoday`,
  repository `specfold`, workflow filename `release.yml`, environment `npm`. Then, under
  Publishing access, require two-factor authentication and disallow tokens, so the workflow is
  the only way to publish. The npm job installs npm 11.5.1, the first release with Trusted
  Publishing, and publishes with `--provenance`.
