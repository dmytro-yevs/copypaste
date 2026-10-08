#!/usr/bin/env bash
# Two isolated histories exchange encrypted rows through a signed native module.
# The backend is a local HTTP fixture, not a Supabase deployment.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
cargo +1.96 test --locked -p copypaste-modules --test supabase -- --nocapture
