#!/usr/bin/env bash
set -euo pipefail
# CI uses the checksum-pinned Linux AMD64 release, not a moving installer.
destination="${1:?usage: install-actionlint.sh destination}"
temporary="$(mktemp -d)"
trap 'rm -rf "$temporary"' EXIT
curl --fail --location --silent --show-error \
  https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_amd64.tar.gz \
  --output "$temporary/actionlint.tar.gz"
printf '%s  %s\n' 8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8 "$temporary/actionlint.tar.gz" | sha256sum --check --status
mkdir -p "$destination"
tar -xzf "$temporary/actionlint.tar.gz" -C "$destination" actionlint
