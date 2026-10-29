#!/usr/bin/env bash
# Tests install.sh without the network: a fake release in a temporary directory, served by file://.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=install.sh
. "$here/install.sh"
set +e
fails=0
ok() { echo "ok   $1"; }
bad() { echo "FAIL $1"; fails=$((fails + 1)); }
check() { if [ "$2" = "$3" ]; then ok "$1"; else bad "$1: expected '$3', got '$2'"; fi; }

check "linux x64" "$(target_for Linux X64)" x86_64-unknown-linux-gnu
check "linux arm64" "$(target_for Linux ARM64)" aarch64-unknown-linux-gnu
check "macos x64" "$(target_for macOS X64)" x86_64-apple-darwin
check "macos arm64" "$(target_for macOS ARM64)" aarch64-apple-darwin
check "windows x64" "$(target_for Windows X64)" x86_64-pc-windows-msvc
if target_for Windows ARM64 >/dev/null 2>&1; then bad "windows arm64 has no build"; else ok "windows arm64 refused"; fi

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
rel="$tmp/release"; mkdir -p "$rel/specfold-x86_64-unknown-linux-gnu"
printf '#!/bin/sh\necho fake spec "$@"\n' > "$rel/specfold-x86_64-unknown-linux-gnu/spec"
chmod +x "$rel/specfold-x86_64-unknown-linux-gnu/spec"
(cd "$rel" && tar -czf specfold-x86_64-unknown-linux-gnu.tar.gz specfold-x86_64-unknown-linux-gnu \
  && sha256_of specfold-x86_64-unknown-linux-gnu.tar.gz | sed 's/$/  specfold-x86_64-unknown-linux-gnu.tar.gz/' > specfold-x86_64-unknown-linux-gnu.tar.gz.sha256)
export SPECFOLD_DOWNLOAD_BASE="file://$rel" SPECFOLD_OS=Linux SPECFOLD_ARCH=X64 GITHUB_PATH="$tmp/path"

bin="$(main v0.0.0 "$tmp/a" 2>&1)" && ok "installs a verified archive" || bad "install: $bin"
check "prints the bin directory" "$bin" "$tmp/a/specfold-x86_64-unknown-linux-gnu"
check "adds it to GITHUB_PATH" "$(cat "$tmp/path")" "$bin"
check "the binary runs" "$("$bin/spec" check)" "fake spec check"

if main "" "$tmp/b" >/dev/null 2>&1; then bad "an empty tag must fail"; else ok "an empty tag is refused"; fi
echo "0000000000000000000000000000000000000000000000000000000000000000  x" > "$rel/specfold-x86_64-unknown-linux-gnu.tar.gz.sha256"
if out="$(main v0.0.0 "$tmp/c" 2>&1)"; then bad "a bad checksum must fail"; else
  case "$out" in *"checksum mismatch"*) ok "a bad checksum is refused" ;; *) bad "checksum message: $out" ;; esac; fi
[ -e "$tmp/c/specfold-x86_64-unknown-linux-gnu" ] && bad "nothing is extracted after a mismatch" || ok "nothing is extracted after a mismatch"
rm "$rel/specfold-x86_64-unknown-linux-gnu.tar.gz.sha256"
if main v0.0.0 "$tmp/d" >/dev/null 2>&1; then bad "a missing checksum must fail"; else ok "a missing checksum is refused"; fi

# The check step of action.yml, run with the fake spec: the document goes to the output file and
# the exit code is passed through.
if command -v python3 >/dev/null 2>&1; then
  python3 -c 'import sys,yaml; print(yaml.safe_load(open(sys.argv[1]))["runs"]["steps"][1]["run"])' "$here/../../action.yml" > "$tmp/step.sh" 2>/dev/null \
    || sed -n '/^      run: |$/,$p' "$here/../../action.yml" | sed '1d; s/^        //' > "$tmp/step.sh"
  mkdir -p "$tmp/w" "$tmp/fakebin"
  printf '#!/bin/sh\necho "{\\"args\\": \\"$*\\"}"\nexit 1\n' > "$tmp/fakebin/spec"; chmod +x "$tmp/fakebin/spec"
  (cd "$tmp/w" && PATH="$tmp/fakebin:$PATH" GITHUB_OUTPUT="$tmp/w/out" SPECFOLD_FORMAT=sarif SPECFOLD_ARGS="--changed origin/main" SPECFOLD_OUTPUT="" bash "$tmp/step.sh" >/dev/null)
  check "the step passes the exit code through" "$?" 1
  check "the step writes the document" "$(cat "$tmp/w/spec-check.sarif.json" 2>/dev/null)" '{"args": "check --format sarif --changed origin/main"}'
  grep -q '^report=spec-check.sarif.json$' "$tmp/w/out" && ok "the step reports the file" || bad "report output: $(cat "$tmp/w/out")"
  (cd "$tmp/w" && GITHUB_OUTPUT="$tmp/w/out" SPECFOLD_FORMAT=html bash "$tmp/step.sh" >/dev/null)
  check "an unknown format is refused" "$?" 2
fi

[ "$fails" -eq 0 ] && echo "all passed" || { echo "$fails failed"; exit 1; }
