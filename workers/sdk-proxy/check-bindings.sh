#!/usr/bin/env bash
set -euo pipefail

config="${1:-wrangler.toml}"

if [[ ! -f "$config" ]]; then
  echo "ConfigurationError: missing $config" >&2
  exit 1
fi

missing=()
placeholder=()

require_text() {
  local name="$1"
  local pattern="$2"
  if ! grep -Eq "$pattern" "$config"; then
    missing+=("$name")
  fi
}

reject_placeholder() {
  local name="$1"
  local pattern="$2"
  if grep -Eq "$pattern" "$config"; then
    placeholder+=("$name")
  fi
}

require_text "GITHUB_APP_ID" 'GITHUB_APP_ID[[:space:]]*='
require_text "GITHUB_INSTALLATION_ID" 'GITHUB_INSTALLATION_ID[[:space:]]*='
require_text "GITHUB_API_CACHE" 'binding[[:space:]]*=[[:space:]]*"GITHUB_API_CACHE"'
require_text "GITHUB_TOKEN_BROKER" 'name[[:space:]]*=[[:space:]]*"GITHUB_TOKEN_BROKER"'
require_text "GithubTokenBroker migration" 'new_sqlite_classes[[:space:]]*=[[:space:]]*\["GithubTokenBroker"\]'

reject_placeholder "GITHUB_APP_ID" 'GITHUB_APP_ID[[:space:]]*=[[:space:]]*"replace-with-'
reject_placeholder "GITHUB_INSTALLATION_ID" 'GITHUB_INSTALLATION_ID[[:space:]]*=[[:space:]]*"replace-with-'
reject_placeholder "GITHUB_API_CACHE id" 'id[[:space:]]*=[[:space:]]*"replace-with-'
reject_placeholder "GITHUB_API_CACHE preview_id" 'preview_id[[:space:]]*=[[:space:]]*"replace-with-'

if [[ -z "${GITHUB_APP_PRIVATE_KEY:-}" ]]; then
  missing+=("GITHUB_APP_PRIVATE_KEY")
fi

if (( ${#missing[@]} || ${#placeholder[@]} )); then
  {
    printf 'ConfigurationError: invalid worker bindings'
    if (( ${#missing[@]} )); then
      printf '\nmissing: %s' "${missing[*]}"
    fi
    if (( ${#placeholder[@]} )); then
      printf '\nplaceholder: %s' "${placeholder[*]}"
    fi
    printf '\n'
  } >&2
  exit 1
fi

echo "worker bindings ok"
