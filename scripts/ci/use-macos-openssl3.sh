#!/usr/bin/env bash
# Select the OpenSSL 3 signing tool required by the module catalog verifier.
set -euo pipefail

[[ "$(uname -s)" == Darwin ]]
if ! brew list --versions openssl@3 >/dev/null 2>&1; then
  brew install openssl@3
fi
module_openssl_prefix="$(brew --prefix openssl@3)"
[[ -x "$module_openssl_prefix/bin/openssl" ]]
"$module_openssl_prefix/bin/openssl" version
printf '%s\n' "$module_openssl_prefix/bin" >> "${GITHUB_PATH:?GitHub Actions path file is required}"
