#!/usr/bin/env bash
# Development shortcuts for CopyPaste
set -euo pipefail

CMD="${1:-help}"

case "$CMD" in
  test)
    echo "Running workspace tests..."
    cd "$(dirname "$0")/.." && docker compose exec dev bash -c "cd /workspace && cargo test --workspace" ;;
  build)
    docker compose exec dev bash -c "cd /workspace && cargo build --workspace" ;;
  check)
    docker compose exec dev bash -c "cd /workspace && cargo check --workspace" ;;
  clippy)
    docker compose exec dev bash -c "cd /workspace && cargo clippy --workspace -- -D warnings" ;;
  fmt)
    docker compose exec dev bash -c "cd /workspace && cargo fmt --all" ;;
  bench)
    docker compose exec dev bash -c "cd /workspace && cargo bench -p copypaste-core --no-run" ;;
  daemon-start)
    echo "Installing and starting daemon..."
    cd "$(dirname "$0")/.." && bash launch/install.sh ;;
  daemon-logs)
    tail -f /tmp/copypaste-daemon.log ;;
  daemon-status)
    echo '{"id":"1","method":"status"}' | nc -U ~/Library/Application\ Support/CopyPaste/daemon.sock 2>/dev/null || echo "daemon not running" ;;
  list)
    echo '{"id":"1","method":"list","params":{"limit":10}}' | nc -U ~/Library/Application\ Support/CopyPaste/daemon.sock 2>/dev/null ;;
  ui-dev)
    cd "$(dirname "$0")/.." && cargo run -p copypaste-ui ;;
  android-build)
    bash "$(dirname "$0")/build-android.sh" ;;
  help|*)
    echo "CopyPaste dev scripts:"
    echo "  test         — cargo test --workspace (in Docker)"
    echo "  build        — cargo build --workspace (in Docker)"
    echo "  check        — cargo check (in Docker)"
    echo "  clippy       — cargo clippy -D warnings (in Docker)"
    echo "  fmt          — cargo fmt --all (in Docker)"
    echo "  bench        — build benchmarks without running"
    echo "  daemon-start — install and start macOS launchd daemon"
    echo "  daemon-logs  — tail daemon log"
    echo "  daemon-status— ping daemon via IPC"
    echo "  list         — show last 10 clipboard items"
    echo "  ui-dev       — run Slint UI (copypaste-ui)"
    echo "  android-build— build Android .so via cargo-ndk"
    ;;
esac
