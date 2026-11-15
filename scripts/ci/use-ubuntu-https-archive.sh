#!/usr/bin/env bash
# Use the authenticated public Ubuntu archive instead of unavailable HTTP mirrors.
set -euo pipefail

for source in /etc/apt/sources.list /etc/apt/sources.list.d/*.list \
              /etc/apt/sources.list.d/*.sources; do
  [[ -f "$source" ]] || continue
  sudo sed -i \
    -e 's|http://azure\.archive\.ubuntu\.com/ubuntu|https://archive.ubuntu.com/ubuntu|g' \
    -e 's|https://azure\.archive\.ubuntu\.com/ubuntu|https://archive.ubuntu.com/ubuntu|g' \
    -e 's|http://archive\.ubuntu\.com/ubuntu|https://archive.ubuntu.com/ubuntu|g' \
    "$source"
done
if [[ -f /etc/apt/apt-mirrors.txt ]]; then
  printf 'https://archive.ubuntu.com/ubuntu/\n' | sudo tee /etc/apt/apt-mirrors.txt >/dev/null
fi
