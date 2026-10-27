#!/usr/bin/env bash
# Publishes the packed release (MODE=publish) or rehearses it with npm publish --dry-run on the
# same paths (any other MODE), so a broken publish fails before a tag. Platform packages go first
# and the main package last; a version already on npm is skipped, which makes a rerun safe.
set -euo pipefail

dir="${1:-./packed}"
# npm reads a bare "packed/x.tgz" as a GitHub owner/repo shorthand.
case "$dir" in
  ./* | /*) ;;
  *)
    echo "publish.sh: the directory must start with ./ or /, not $dir" >&2
    exit 2
    ;;
esac

root="$(cd "$(dirname "$0")/.." && pwd)"
release="$(node -p 'require(process.argv[1]).version' "$root/package.json")"
rehearse=true
if [ "${MODE:-}" = publish ]; then rehearse=false; fi
published=0

publish() {
  local manifest name version
  manifest="$(tar -xzOf "$1" package/package.json)"
  name="$(node -p 'JSON.parse(process.argv[1]).name' "$manifest")"
  version="$(node -p 'JSON.parse(process.argv[1]).version' "$manifest")"
  if [ "$(npm view "$name@$version" version 2>/dev/null)" = "$version" ]; then
    echo "skip: $name@$version is already on npm"
  elif [ "$rehearse" = true ]; then
    npm publish "$1" --access public --dry-run
  else
    npm publish "$1" --access public --provenance
    published=$((published + 1))
  fi
}

platforms=("$dir"/modootoday-specfold-linux-*.tgz "$dir"/modootoday-specfold-darwin-*.tgz "$dir"/modootoday-specfold-win32-*.tgz)
for archive in "${platforms[@]}"; do
  publish "$archive"
done

# Published before npm answers for every platform package, the main package would name
# versions an install cannot fetch yet, and that install would get no binary without a word.
if [ "$rehearse" = false ]; then
  deadline=$(($(date +%s) + ${PLATFORM_WAIT_SECONDS:-3600}))
  for archive in "${platforms[@]}"; do
    manifest="$(tar -xzOf "$archive" package/package.json)"
    name="$(node -p "JSON.parse(process.argv[1]).name" "$manifest")"
    until [ "$(npm view "$name@$release" version 2>/dev/null)" = "$release" ]; do
      if [ "$(date +%s)" -ge "$deadline" ]; then
        echo "publish.sh: npm still does not answer for $name@$release; the main package was not published. Run again once it does." >&2
        echo "published=$published" >> "${GITHUB_OUTPUT:-/dev/null}"
        exit 1
      fi
      echo "waiting for npm to answer for $name@$release"
      sleep "${PLATFORM_POLL_SECONDS:-30}"
    done
  done
fi
publish "$dir/modootoday-specfold-$release.tgz"

echo "published=$published" >> "${GITHUB_OUTPUT:-/dev/null}"
if [ "$rehearse" = true ]; then
  echo "rehearsal: every package passed npm publish --dry-run"
fi
