#!/usr/bin/env bash
# Install PostgreSQL server binaries for supabase/dev/verify-schema.sh.
#
# That script needs initdb/pg_ctl/psql and starts its own throwaway cluster
# (docs/supabase-deployment.md). It must not start a system service.
#
# Usage (CI or local Ubuntu):
#   scripts/ci/install-postgresql-server.sh
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

# Bound wall-clock so a stuck apt fails the job instead of sitting for an hour.
APT_TIMEOUT_SECS="${APT_TIMEOUT_SECS:-300}"

# GitHub-hosted ubuntu images point apt at azure.archive.ubuntu.com. That
# mirror intermittently hangs (Ign: … then silence until the job times out).
# Force the public archive before any apt call.
prefer_public_ubuntu_archive() {
  local f
  for f in /etc/apt/sources.list /etc/apt/sources.list.d/*.list \
           /etc/apt/sources.list.d/*.sources; do
    [[ -f "$f" ]] || continue
    sudo sed -i \
      -e 's|http://azure\.archive\.ubuntu\.com/ubuntu|http://archive.ubuntu.com/ubuntu|g' \
      -e 's|https://azure\.archive\.ubuntu\.com/ubuntu|http://archive.ubuntu.com/ubuntu|g' \
      "$f"
  done
  if [[ -f /etc/apt/apt-mirrors.txt ]]; then
    printf 'http://archive.ubuntu.com/ubuntu/\n' | sudo tee /etc/apt/apt-mirrors.txt >/dev/null
  fi
}

prefer_public_ubuntu_archive

# Pin a versioned server package: the meta package pulls cluster auto-setup.
# Ubuntu 24.04 ships 16; fall back if the image only has another major.
pkg="$(apt-cache search --names-only '^postgresql-[0-9]+$' 2>/dev/null \
  | awk '{print $1}' | sort -V | tail -1 || true)"
if [[ -z "$pkg" ]]; then
  pkg=postgresql-16
fi

echo "install-postgresql-server: installing $pkg (timeout ${APT_TIMEOUT_SECS}s)"

timeout "$APT_TIMEOUT_SECS" sudo apt-get update -y
# No cluster, no service: verify-schema.sh owns initdb.
echo 'postgresql-common postgresql-common/create-cluster boolean false' \
  | sudo debconf-set-selections
echo 'postgresql-common postgresql-common/auto-start boolean false' \
  | sudo debconf-set-selections

timeout "$APT_TIMEOUT_SECS" sudo apt-get install -y --no-install-recommends \
  "$pkg" postgresql-client

bindir="$(ls -d /usr/lib/postgresql/*/bin 2>/dev/null | sort -V | tail -1 || true)"
if [[ -z "$bindir" || ! -x "$bindir/initdb" ]]; then
  echo "install-postgresql-server: no initdb after install" >&2
  exit 2
fi
echo "install-postgresql-server: binaries in $bindir"
