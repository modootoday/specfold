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

# run.sh, the check step, with a fake spec that echoes its arguments and exits $FAKE_EXIT.
mkdir -p "$tmp/fakebin" "$tmp/w"
printf '#!/bin/sh\necho "$*"\nexit ${FAKE_EXIT:-1}\n' > "$tmp/fakebin/spec"; chmod +x "$tmp/fakebin/spec"
step() { (cd "${STEP_DIR:-$tmp/w}" && env PATH="$tmp/fakebin:$PATH" GITHUB_OUTPUT="$tmp/w/out" "$@" bash "$here/run.sh"); }
: > "$tmp/w/out"
step SPECFOLD_FORMAT=sarif SPECFOLD_ARGS="--changed origin/main" >/dev/null
check "the step passes the exit code through" "$?" 1
check "the step writes the document" "$(cat "$tmp/w/spec-check.sarif.json")" "check --format sarif --changed origin/main"
grep -q '^report=spec-check.sarif.json$' "$tmp/w/out" && ok "the step reports the file" || bad "report output: $(cat "$tmp/w/out")"
step SPECFOLD_FORMAT=html >/dev/null
check "an unknown format is refused" "$?" 2
check "fail-on becomes --fail-on flags" "$(step SPECFOLD_FAIL_ON="L1, S4")" "check --format github --fail-on L1 --fail-on S4"
step SPECFOLD_FAIL_ON=none >/dev/null
check "fail-on none passes findings" "$?" 0
step SPECFOLD_FAIL_ON=none FAKE_EXIT=2 >/dev/null
check "fail-on none still fails a usage error" "$?" 2
check "changed outside a pull request checks everything" "$(step GITHUB_BASE_REF=)" "check --format github"
check "an explicit changed revision" "$(step SPECFOLD_CHANGED=v1.0)" "check --format github --changed v1.0"

# changed: auto in a pull request: the merge base, also from a shallow clone.
g() { git -c user.name=t -c user.email=t@t -c init.defaultBranch=main "$@"; }
up="$tmp/up"; g init -q "$up"
for i in 1 2 3 4 5; do echo "$i" > "$up/f"; g -C "$up" add f; g -C "$up" commit -qm "c$i"; done
g -C "$up" checkout -qb topic HEAD~2; echo t > "$up/t"; g -C "$up" add t; g -C "$up" commit -qm topic
mb="$(g -C "$up" merge-base main topic)"
g clone -q --branch topic "file://$up" "$tmp/full"
check "a pull request checks from its merge base" "$(STEP_DIR="$tmp/full" step GITHUB_BASE_REF=main | tail -1)" "check --format github --changed $mb"
g clone -q --depth 1 --branch topic "file://$up" "$tmp/shallow"
check "a shallow clone is deepened to the merge base" "$(STEP_DIR="$tmp/shallow" step GITHUB_BASE_REF=main | tail -1)" "check --format github --changed $mb"
check "args with --staged keep their own range" "$(STEP_DIR="$tmp/full" step GITHUB_BASE_REF=main SPECFOLD_ARGS=--staged)" "check --format github --staged"

[ "$fails" -eq 0 ] && echo "all passed" || { echo "$fails failed"; exit 1; }
