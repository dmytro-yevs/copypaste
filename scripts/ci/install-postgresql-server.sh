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

# Keep package installation on the authenticated public archive. HTTP mirrors
# have failed before product tests start; transport changes do not alter checks.
bash "$(dirname "${BASH_SOURCE[0]}")/use-ubuntu-https-archive.sh"

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
