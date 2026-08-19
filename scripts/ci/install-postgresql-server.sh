#!/usr/bin/env bash
# Install PostgreSQL server binaries for supabase/dev/verify-schema.sh.
#
# That script needs initdb/pg_ctl/psql and starts its own throwaway cluster
# (docs/supabase-deployment.md). It must not start a system service — the
# meta `postgresql` package used to hang GHA runners on debconf/cluster setup.
#
# Usage (CI or local Ubuntu):
#   scripts/ci/install-postgresql-server.sh
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

# Bound wall-clock so a stuck apt/mirror fails the job instead of sitting for
# an hour under cancel-in-progress noise.
APT_TIMEOUT_SECS="${APT_TIMEOUT_SECS:-600}"

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
