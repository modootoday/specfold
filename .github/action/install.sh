#!/usr/bin/env bash
# Downloads a specfold release archive, checks it against its .sha256 file and puts `spec` on
# PATH. Used by action.yml; runnable alone, and tested by .github/action/test.sh.
#
#   install.sh <tag> [dest]
#
# SPECFOLD_REPOSITORY (owner/name, default modootoday/specfold) picks the repository;
# SPECFOLD_DOWNLOAD_BASE replaces the whole download URL prefix (tests point it at file://).
# SPECFOLD_OS / SPECFOLD_ARCH override the runner's RUNNER_OS / RUNNER_ARCH.
set -euo pipefail

# Maps a runner (RUNNER_OS, RUNNER_ARCH as GitHub names them) to a release target.
target_for() {
  case "$1/$2" in
    Linux/X64) echo x86_64-unknown-linux-gnu ;;
    Linux/ARM64) echo aarch64-unknown-linux-gnu ;;
    macOS/X64) echo x86_64-apple-darwin ;;
    macOS/ARM64) echo aarch64-apple-darwin ;;
    Windows/X64) echo x86_64-pc-windows-msvc ;;
    *) echo "specfold has no release build for $1/$2" >&2; return 1 ;;
  esac
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# The release to install: the version input, else the ref the action itself was used at when
# that ref is a release tag (uses: owner/specfold@v1.2.3). A branch, a commit SHA or a local
# `uses: ./` has no matching release, so those need the version input.
resolve_tag() {
  local tag="${1:-}" ref="${2:-}"
  if [ -n "$tag" ]; then echo "$tag"; return 0; fi
  if printf '%s' "$ref" | grep -Eq '^v?[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$'; then
    echo "$ref"; return 0
  fi
  local how="the action ref '${ref}'"; [ -z "$ref" ] && how="a local or unknown action ref"
  echo "no version input, and $how is not a release tag; pin the action to a release tag (uses: ${SPECFOLD_REPOSITORY:-modootoday/specfold}@v1.2.3) or set version (tags on https://github.com/${SPECFOLD_REPOSITORY:-modootoday/specfold}/releases)" >&2
  return 1
}

main() {
  local tag dest="${2:-${RUNNER_TEMP:-/tmp}/specfold}"
  tag="$(resolve_tag "${1:-}" "${SPECFOLD_ACTION_REF:-}")" || return 1
  case "$tag" in v*) ;; *) tag="v$tag" ;; esac
  local os="${SPECFOLD_OS:-${RUNNER_OS:-}}" arch="${SPECFOLD_ARCH:-${RUNNER_ARCH:-}}"
  local target; target="$(target_for "$os" "$arch")"
  local ext=tar.gz; case "$target" in *windows*) ext=zip ;; esac
  local name="specfold-$target"
  local base="${SPECFOLD_DOWNLOAD_BASE:-https://github.com/${SPECFOLD_REPOSITORY:-modootoday/specfold}/releases/download/$tag}"

  mkdir -p "$dest"
  curl -fsSL --retry 3 -o "$dest/$name.$ext" "$base/$name.$ext"
  curl -fsSL --retry 3 -o "$dest/$name.$ext.sha256" "$base/$name.$ext.sha256"
  local want got
  want="$(cut -d' ' -f1 < "$dest/$name.$ext.sha256" | tr -d '\r\n' | tr 'A-F' 'a-f')"
  got="$(sha256_of "$dest/$name.$ext")"
  if [ -z "$want" ] || [ "$want" != "$got" ]; then
    echo "checksum mismatch for $name.$ext: expected '$want', got '$got'" >&2
    return 1
  fi
  case "$ext" in
    zip) if command -v unzip >/dev/null 2>&1; then unzip -qo "$dest/$name.$ext" -d "$dest"; else tar -xf "$dest/$name.$ext" -C "$dest"; fi ;;
    *) tar -xzf "$dest/$name.$ext" -C "$dest" ;;
  esac
  local bin="$dest/$name"
  [ -f "$bin/spec" ] || [ -f "$bin/spec.exe" ] || { echo "the archive has no spec binary under $name/" >&2; return 1; }
  chmod +x "$bin"/spec* "$bin"/specfold* 2>/dev/null || true
  if [ -n "${GITHUB_PATH:-}" ]; then echo "$bin" >> "$GITHUB_PATH"; fi
  echo "$bin"
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then main "$@"; fi
