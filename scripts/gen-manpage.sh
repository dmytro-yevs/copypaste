#!/usr/bin/env bash
# gen-manpage.sh — Generate man/copypaste.1 from CLI --help output.
#
# Strategy (in order of preference, no cargo deps added):
#   1. help2man          (recommended; produces canonical groff)
#   2. Hand-rolled fallback (parses --help, fills man/copypaste.1.in template)
#
# Output: man/copypaste.1
#
# Install: see docs/man/README.md

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TEMPLATE="${ROOT}/man/copypaste.1.in"
OUTPUT="${ROOT}/man/copypaste.1"
BIN="copypaste"
VERSION="$(grep -E '^version' "${ROOT}/crates/copypaste-cli/Cargo.toml" 2>/dev/null | head -1 | cut -d'"' -f2 || echo "0.2.0")"
DATE="$(date +%Y-%m-%d)"

log() { printf "[gen-manpage] %s\n" "$*" >&2; }

# Build CLI once so --help is fast and consistent.
log "building copypaste-cli (release)..."
(cd "${ROOT}" && cargo build -p copypaste-cli --quiet --release) || {
    log "WARN: release build failed; trying debug"
    (cd "${ROOT}" && cargo build -p copypaste-cli --quiet) || {
        log "ERROR: cargo build failed"
        exit 1
    }
}

# Locate the built binary
CARGO_BIN=""
for candidate in "${ROOT}/target/release/copypaste" "${ROOT}/target/debug/copypaste"; do
    if [ -x "${candidate}" ]; then
        CARGO_BIN="${candidate}"
        break
    fi
done

if [ -z "${CARGO_BIN}" ]; then
    log "ERROR: copypaste binary not found in target/"
    exit 1
fi

log "using binary: ${CARGO_BIN}"

# Prefer help2man if available
if command -v help2man >/dev/null 2>&1; then
    log "found help2man — generating canonical man page"
    help2man \
        --name="clipboard history CLI" \
        --section=1 \
        --no-info \
        --source="CopyPaste ${VERSION}" \
        --output="${OUTPUT}" \
        "${CARGO_BIN}"
    log "wrote ${OUTPUT}"
    exit 0
fi

# Fallback: hand-rolled from template
log "help2man not found — using template fallback (install help2man for richer output)"

if [ ! -f "${TEMPLATE}" ]; then
    log "ERROR: template missing at ${TEMPLATE}"
    exit 1
fi

# Capture top-level help and subcommand list
HELP_TXT="$("${CARGO_BIN}" --help 2>&1 || true)"

# Extract one-line summaries for each subcommand from the Commands: block
SUBCMDS="$(printf '%s\n' "${HELP_TXT}" | awk '
    /^Commands:/ { in_cmds=1; next }
    in_cmds && /^[A-Za-z]+:/ { in_cmds=0 }
    in_cmds && /^  [a-z]/ {
        sub(/^  /, "")
        # split first whitespace run
        n = split($0, parts, /[ \t]+/)
        cmd = parts[1]
        desc = ""
        for (i = 2; i <= n; i++) desc = (desc == "" ? parts[i] : desc " " parts[i])
        printf ".TP\n.B %s\n%s\n", cmd, desc
    }
')"

# Substitute placeholders in the template
sed \
    -e "s/@VERSION@/${VERSION}/g" \
    -e "s/@DATE@/${DATE}/g" \
    "${TEMPLATE}" > "${OUTPUT}.tmp"

# Insert subcommand block after the @COMMANDS@ marker line
awk -v cmds="${SUBCMDS}" '
    /@COMMANDS@/ { print cmds; next }
    { print }
' "${OUTPUT}.tmp" > "${OUTPUT}"

rm -f "${OUTPUT}.tmp"

log "wrote ${OUTPUT} (fallback mode)"
log "tip: install help2man (brew install help2man) for canonical formatting"
