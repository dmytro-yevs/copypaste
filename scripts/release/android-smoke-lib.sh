#!/usr/bin/env bash
# android-smoke-lib.sh — the verdicts, the detectors, and their self-test.
#
# Sourced by android-smoke.sh, which is the sequence that drives a device. The
# split is rule 5's: what counts as a crash, as an unencrypted database or as
# leaked plaintext is one responsibility, and it is the only one that can be
# proved on a machine with no Android SDK — `android-smoke.sh --self-test` runs
# [self_test] below against fixtures, and check.sh runs that.
#
# Three verdicts, and the difference between them is the point:
#
#   ok / FAIL      an assertion; a FAIL fails the job
#   NOT ASSERTED   this environment cannot observe it — printed, never skipped
#   probe          an observation kept for the next round, not a gate
set -uo pipefail

PKG="${PKG:-com.copypaste.app}"
OUT="${SMOKE_OUT:-artifacts/android-smoke}"

PASS=0
FAIL=0
NOTES=()
PROBES=()

ok()   { PASS=$((PASS + 1)); printf '  ok    %s\n' "$1"; }
bad()  { FAIL=$((FAIL + 1)); printf '  FAIL  %s\n' "$1"
         [[ -n "${2:-}" ]] && printf '        %s\n' "$2"; return 0; }
note() { NOTES+=("$1 — $2"); printf '  ----  NOT ASSERTED  %s\n              %s\n' "$1" "$2"; }
probe(){ PROBES+=("$1 -> $2"); printf '  ....  probe  %s -> %s\n' "$1" "$2"; }
group(){ printf '\n== %s\n' "$1"; }

# ---------------------------------------------------------------------------
# Detection logic. Everything here is pure: a file in, a verdict out, so
# --self-test can prove each one actually fails when it should.
# ---------------------------------------------------------------------------

# `Process … has died` is deliberately not here: force-stop, which this script
# does on purpose between launches, prints it.
CRASH_PATTERNS='FATAL EXCEPTION|E AndroidRuntime|UnsatisfiedLinkError|Fatal signal|beginning of crash|did not include required runtime symbols'

# The crash blocks in a logcat dump that belong to *this* app.
#
# A crash elsewhere on the emulator is not ours, so the block has to name us —
# and libc truncates the process to 15 characters ("m.copypaste.app"), which is
# why the token matched is `copypaste` rather than the package.
crash_report() {
    awk -v pat="$CRASH_PATTERNS" '
        $0 ~ pat && left == 0 { left = 14; block = $0 "\n"; hit = (tolower($0) ~ /copypaste/); next }
        left > 0 {
            block = block $0 "\n"
            if (tolower($0) ~ /copypaste/) hit = 1
            if (--left == 0 && hit) printf "%s", block
        }
        END { if (left > 0 && hit) printf "%s", block }
    ' "$1"
}

# A history database that opens without SQLCipher is the failure ADR-0007 is
# about: it would be readable clipboard plaintext at rest.
looks_encrypted() {
    [[ -s "$1" ]] || return 1
    local magic
    magic="$(head -c 15 "$1" | tr -d '\000')"
    [[ "$magic" != "SQLite format 3" ]]
}

# SQLCipher's page 1 opens with a per-database random salt. It survives every
# write and changes only when the file is recreated, which makes it the one
# cheap proof that the *same* database was reopened.
salt_of() { od -An -v -tx1 -N16 "$1" | tr -d ' \n'; }

holds_text() { grep -aqF "$2" "$1"; }

# ---------------------------------------------------------------------------
# adb helpers
# ---------------------------------------------------------------------------

sh_() { adb shell "$@" 2>&1 | tr -d '\r'; }
app_pid() { adb shell pidof "$PKG" 2>/dev/null | tr -d '\r' | awk '{print $1}'; }
has_pid() { [[ -n "$(app_pid)" ]]; }
no_pid()  { [[ -z "$(app_pid)" ]]; }

# Wait up to <secs> for a *function* to succeed. It has to be a function: a
# command built here would have its arguments expanded once, before the wait.
wait_for() {
    local secs="$1"; shift
    local i=0
    while (( i < secs )); do
        "$@" >/dev/null 2>&1 && return 0
        sleep 1
        i=$((i + 1))
    done
    return 1
}

dump_logcat() { adb logcat -d -b all > "$OUT/$1.log" 2>&1 || true; }

# Everything under the app's private directory, addressed the way run-as sees
# it: run-as chdir's to the data directory, so these paths stay relative.
app_files() { adb shell run-as "$PKG" find . -type f 2>/dev/null | tr -d '\r'; }

pull_file() {
    local remote="$1" local_="$2"
    adb exec-out run-as "$PKG" cat "$remote" > "$local_" 2>/dev/null
    [[ -s "$local_" ]] || { rm -f "$local_"; return 1; }
}

# The database is in WAL mode, so a write can land entirely in the -wal file
# and leave the main database byte-identical. Anything asking "did the store
# change?" has to hash the set.
DB_REL=""
db_fingerprint() {
    local tag="$1" f base out="$OUT/dbset-$1.txt"
    : > "$out"
    for f in "$DB_REL" "${DB_REL}-wal"; do
        base="$(basename "$f")"
        if pull_file "$f" "$OUT/${tag}-${base}"; then
            printf '%s  %s\n' "$(sha256sum < "$OUT/${tag}-${base}" | cut -d' ' -f1)" "$base" >> "$out"
        else
            printf '%s  %s\n' "absent" "$base" >> "$out"
        fi
    done
    cat "$out"
}

# ---------------------------------------------------------------------------
# --self-test: prove the detectors above fail when they should
# ---------------------------------------------------------------------------

self_test() {
    local t
    SELF_TEST_TMP="$(mktemp -d)"
    t="$SELF_TEST_TMP"
    trap 'rm -rf "$SELF_TEST_TMP"' EXIT
    group "self-test: crash detection"

    printf 'I ActivityManager: Start proc 1234:com.copypaste.app/u0a123\nI copypaste: hello\n' > "$t/clean.log"
    [[ -z "$(crash_report "$t/clean.log")" ]] \
        && ok "an ordinary log naming the app is not a crash" \
        || bad "an ordinary log naming the app is not a crash" "$(crash_report "$t/clean.log")"

    printf 'E AndroidRuntime: FATAL EXCEPTION: main\nE AndroidRuntime: Process: com.copypaste.app, PID: 4242\nE AndroidRuntime: java.lang.RuntimeException\n' > "$t/ours.log"
    [[ -n "$(crash_report "$t/ours.log")" ]] \
        && ok "a FATAL EXCEPTION naming our process is reported" \
        || bad "a FATAL EXCEPTION naming our process is reported"

    printf 'E AndroidRuntime: FATAL EXCEPTION: main\nE AndroidRuntime: Process: com.android.settings, PID: 99\n' > "$t/theirs.log"
    [[ -z "$(crash_report "$t/theirs.log")" ]] \
        && ok "another package's crash is not ours" \
        || bad "another package's crash is not ours" "$(crash_report "$t/theirs.log")"

    printf 'F libc: Fatal signal 11 (SIGSEGV) in tid 4242 (m.copypaste.app)\n' > "$t/libc.log"
    [[ -n "$(crash_report "$t/libc.log")" ]] \
        && ok "libc's truncated process name still matches" \
        || bad "libc's truncated process name still matches"

    printf 'E AndroidRuntime: java.lang.UnsatisfiedLinkError: couldnt find "libcopypaste_ui_lib.so"\n' > "$t/link.log"
    [[ -n "$(crash_report "$t/link.log")" ]] \
        && ok "a missing libcopypaste_ui_lib.so is reported" \
        || bad "a missing libcopypaste_ui_lib.so is reported"

    group "self-test: database detection"

    printf 'SQLite format 3\000rest of an unencrypted database' > "$t/plain.db"
    looks_encrypted "$t/plain.db" \
        && bad "a plain SQLite header is not accepted as encrypted" \
        || ok "a plain SQLite header is not accepted as encrypted"

    head -c 4096 /dev/urandom > "$t/cipher.db"
    looks_encrypted "$t/cipher.db" \
        && ok "a SQLCipher-shaped file is accepted" \
        || bad "a SQLCipher-shaped file is accepted"

    looks_encrypted "$t/missing.db" \
        && bad "an absent database is not accepted" \
        || ok "an absent database is not accepted"

    local salt
    salt="$(salt_of "$t/cipher.db")"
    [[ ${#salt} -eq 32 ]] \
        && ok "salt_of reads 16 bytes as hex" \
        || bad "salt_of reads 16 bytes as hex" "got ${#salt} characters"

    printf 'binary\000CANARY-42\000more' > "$t/blob"
    holds_text "$t/blob" "CANARY-42" \
        && ok "a canary in a binary file is found" \
        || bad "a canary in a binary file is found"
    holds_text "$t/cipher.db" "CANARY-42" \
        && bad "a canary absent from a file is not found" \
        || ok "a canary absent from a file is not found"

    printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
    [[ $FAIL -eq 0 ]]
}

# ---------------------------------------------------------------------------
# The summary, printed on every exit path
# ---------------------------------------------------------------------------

summary() {
    local verdict="FAILED"
    [[ $FAIL -eq 0 ]] && verdict="passed"
    {
        printf '\n## Android emulator smoke test: %s\n\n' "$verdict"
        printf '%d assertions passed, %d failed.\n\n' "$PASS" "$FAIL"
        if [[ ${#NOTES[@]} -gt 0 ]]; then
            printf 'Not asserted — this emulator cannot observe these:\n\n'
            printf '* %s\n' "${NOTES[@]}"
            printf '\n'
        fi
        if [[ ${#PROBES[@]} -gt 0 ]]; then
            printf 'Observations, recorded but not gating:\n\n'
            printf '* %s\n' "${PROBES[@]}"
            printf '\n'
        fi
    } | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
}
