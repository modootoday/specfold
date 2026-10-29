#!/usr/bin/env bash
# The check step of action.yml: runs spec check in the chosen format and passes its exit code on.
# Reads SPECFOLD_FORMAT, SPECFOLD_ARGS, SPECFOLD_OUTPUT, SPECFOLD_FAIL_ON and SPECFOLD_CHANGED;
# writes report= and exit-code= to GITHUB_OUTPUT. Tested by .github/action/test.sh.
set -uo pipefail

# The commit a pull request branched from, so --changed sees only the pull request's own
# changes. Fetches the base branch, and deepens a shallow checkout until the two histories meet.
pr_base() {
  local base="$1" ref="refs/remotes/origin/$1" mb
  git fetch -q --no-tags origin "+refs/heads/$base:$ref" 2>/dev/null || true
  for depth in 50 200 1000; do
    if mb="$(git merge-base "$ref" HEAD 2>/dev/null)"; then echo "$mb"; return 0; fi
    [ "$(git rev-parse --is-shallow-repository 2>/dev/null)" = true ] || break
    git fetch -q --no-tags --deepen="$depth" origin HEAD "+refs/heads/$base:$ref" 2>/dev/null || break
  done
  if [ "$(git rev-parse --is-shallow-repository 2>/dev/null)" = true ]; then
    git fetch -q --no-tags --unshallow origin 2>/dev/null || true
    git fetch -q --no-tags origin "+refs/heads/$base:$ref" 2>/dev/null || true
  fi
  mb="$(git merge-base "$ref" HEAD 2>/dev/null)" || return 1
  echo "$mb"
}

main() {
  local format="${SPECFOLD_FORMAT:-github}" changed="${SPECFOLD_CHANGED-auto}" fail_on="${SPECFOLD_FAIL_ON:-}"
  case "$format" in github|sarif|rdjson|gitlab) ;; *) echo "::error::format must be github, sarif, rdjson or gitlab"; return 2 ;; esac
  # Word-split args on purpose: they are flags for spec check.
  # shellcheck disable=SC2086
  set -- check --format "$format" ${SPECFOLD_ARGS:-}
  local given=" $* "
  case "$given" in *" --changed"*|*" --staged "*) changed=off ;; esac
  case "$changed" in
    off|false|"") ;;
    auto)
      if [ -n "${GITHUB_BASE_REF:-}" ]; then
        local mb
        if mb="$(pr_base "$GITHUB_BASE_REF")"; then
          echo "checking what the pull request changes since $mb (the merge base with $GITHUB_BASE_REF)"
          set -- "$@" --changed "$mb"
        else
          echo "::warning::no merge base with $GITHUB_BASE_REF; checking every document"
        fi
      fi ;;
    *) set -- "$@" --changed "$changed" ;;
  esac
  local never=false rule
  for rule in ${fail_on//,/ }; do
    if [ "$rule" = none ]; then never=true; else set -- "$@" --fail-on "$rule"; fi
  done
  local code=0 out=""
  if [ "$format" = github ]; then
    spec "$@" || code=$?
  else
    out="${SPECFOLD_OUTPUT:-spec-check.$format.json}"
    spec "$@" > "$out" || code=$?
    echo "wrote $out"
  fi
  if [ -n "${GITHUB_OUTPUT:-}" ]; then
    { echo "report=$out"; echo "exit-code=$code"; } >> "$GITHUB_OUTPUT"
  fi
  # Findings exit 1; anything else (a bad argument, a broken config) fails even with fail-on: none.
  if [ "$never" = true ] && [ "$code" = 1 ]; then return 0; fi
  return "$code"
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then main "$@"; fi
