bridge_lock_dir="$repo_dir/target/copypaste-dev/web-bridge.lock"
bridge_runtime="$ui_dir/public/copypaste-web-bridge.js"
bridge_lock_owned=false
bridge_runtime_owned=false

acquire_bridge_session() {
  mkdir -p "$(dirname "$bridge_lock_dir")"
  if mkdir "$bridge_lock_dir" 2>/dev/null; then
    bridge_lock_owned=true
    printf '%s\n' "$$" >"$bridge_lock_dir/pid"
    return 0
  fi

  bridge_owner_pid=$(sed -n '1p' "$bridge_lock_dir/pid" 2>/dev/null || true)
  case "$bridge_owner_pid" in
    ''|*[!0-9]*) bridge_owner_pid='' ;;
  esac
  if [ -n "$bridge_owner_pid" ] && kill -0 "$bridge_owner_pid" 2>/dev/null; then
    echo "CopyPaste browser bridge is already managed by process $bridge_owner_pid."
    return 1
  fi

  rm -f "$bridge_lock_dir/pid"
  if ! rmdir "$bridge_lock_dir" 2>/dev/null || ! mkdir "$bridge_lock_dir" 2>/dev/null; then
    echo "CopyPaste browser bridge session is already being claimed." >&2
    return 1
  fi
  bridge_lock_owned=true
  printf '%s\n' "$$" >"$bridge_lock_dir/pid"
}

write_bridge_runtime_value() {
  bridge_runtime_tmp=$(mktemp "$ui_dir/public/.copypaste-web-bridge.XXXXXX")
  chmod 600 "$bridge_runtime_tmp"
  printf 'window.__COPYPASTE_WEB_BRIDGE__ = %s;\n' "$1" >"$bridge_runtime_tmp"
  mv -f "$bridge_runtime_tmp" "$bridge_runtime"
}

write_bridge_runtime() {
  write_bridge_runtime_value "$(printf '{\"url\":\"%s\",\"token\":\"%s\"}' \
    "$VITE_COPYPASTE_WEB_BRIDGE_URL" \
    "$VITE_COPYPASTE_WEB_BRIDGE_TOKEN")"
  bridge_runtime_owned=true
}

clear_bridge_runtime() {
  if [ "$bridge_runtime_owned" != true ]; then
    return
  fi
  write_bridge_runtime_value 'null'
  bridge_runtime_owned=false
}

release_bridge_session() {
  if [ "$bridge_lock_owned" != true ]; then
    return
  fi
  bridge_recorded_pid=$(sed -n '1p' "$bridge_lock_dir/pid" 2>/dev/null || true)
  if [ "$bridge_recorded_pid" = "$$" ]; then
    rm -f "$bridge_lock_dir/pid"
    rmdir "$bridge_lock_dir" 2>/dev/null || true
  fi
  bridge_lock_owned=false
}

wait_for_daemon() {
  daemon_wait_seconds=${COPYPASTE_DAEMON_START_TIMEOUT_SECONDS:-120}
  case "$daemon_wait_seconds" in
    ''|*[!0-9]*)
      echo "COPYPASTE_DAEMON_START_TIMEOUT_SECONDS must be a whole number." >&2
      return 1
      ;;
  esac

  daemon_attempt=0
  daemon_max_attempts=$((daemon_wait_seconds * 10))
  while ! "$cli_bin" status >/dev/null 2>&1; do
    if [ "$daemon_owned" = true ] && ! kill -0 "$daemon_pid" 2>/dev/null; then
      echo "CopyPaste daemon exited before it became ready." >&2
      return 1
    fi
    if [ "$daemon_attempt" -ge "$daemon_max_attempts" ]; then
      echo "CopyPaste daemon did not become ready within ${daemon_wait_seconds}s." >&2
      return 1
    fi
    daemon_attempt=$((daemon_attempt + 1))
    sleep 0.1
  done
}

wait_for_bridge_runtime() {
  bridge_wait_seconds=${COPYPASTE_WEB_BRIDGE_START_TIMEOUT_SECONDS:-120}
  case "$bridge_wait_seconds" in
    ''|*[!0-9]*)
      echo "COPYPASTE_WEB_BRIDGE_START_TIMEOUT_SECONDS must be a whole number." >&2
      return 1
      ;;
  esac

  bridge_attempt=0
  bridge_max_attempts=$((bridge_wait_seconds * 10))
  while [ ! -s "$bridge_env" ]; do
    if ! kill -0 "$bridge_pid" 2>/dev/null; then
      echo "CopyPaste browser bridge exited before publishing its runtime." >&2
      return 1
    fi
    if [ "$bridge_attempt" -ge "$bridge_max_attempts" ]; then
      echo "CopyPaste browser bridge did not start within ${bridge_wait_seconds}s." >&2
      return 1
    fi
    bridge_attempt=$((bridge_attempt + 1))
    sleep 0.1
  done
}
