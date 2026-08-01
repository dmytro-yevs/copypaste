#!/usr/bin/env bash
# setup-tap.sh — bootstrap the Homebrew tap repository.
#
#   scripts/release/setup-tap.sh --github-user <user> [--tap-name copypaste] [--dry-run]
#
# Creates ../homebrew-<tap-name>/ containing the cask, the formula and a
# README, initialised as a git repository. It never pushes and never creates
# anything on GitHub: the maintainer does that, with their own credentials, and
# the next steps are printed at the end.
#
# Run once. After that the release workflow publishes into the tap directly and
# this script has no further part to play.
#
# Why a tap at all: homebrew/cask's audit requires casks to be code-signed and
# notarised, and casks failing that check are removed from the core tap on
# 2026-09-01. We have no Apple Developer ID (ADR-0001), so the core tap is
# closed to us. A third-party tap runs no such audit.
set -euo pipefail

TAP_NAME="copypaste"
GITHUB_USER=""
DRY_RUN=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --github-user) [[ $# -ge 2 ]] || { echo "ERROR: --github-user needs a value" >&2; exit 1; }
                       GITHUB_USER="$2"; shift 2 ;;
        --tap-name)    [[ $# -ge 2 ]] || { echo "ERROR: --tap-name needs a value" >&2; exit 1; }
                       TAP_NAME="$2"; shift 2 ;;
        --dry-run)     DRY_RUN=1; shift ;;
        -h|--help)     sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "ERROR: unknown argument: $1" >&2; exit 1 ;;
    esac
done

[[ -n "$GITHUB_USER" ]] || { echo "ERROR: --github-user is required" >&2; exit 1; }
[[ "$TAP_NAME" =~ ^[a-z0-9][a-z0-9-]*$ ]] \
    || { echo "ERROR: --tap-name must be lowercase alphanumeric and hyphens" >&2; exit 1; }
[[ "$GITHUB_USER" =~ ^[a-zA-Z0-9]([a-zA-Z0-9-]{0,38}[a-zA-Z0-9])?$ ]] \
    || { echo "ERROR: --github-user is not a valid GitHub username" >&2; exit 1; }

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TAP_DIR="$(cd "$REPO_ROOT/.." && pwd)/homebrew-${TAP_NAME}"

CASK="$REPO_ROOT/Casks/copypaste.rb"
FORMULA="$REPO_ROOT/packaging/homebrew/copypaste-cli.rb"
[[ -f "$CASK" ]]    || { echo "ERROR: $CASK not found" >&2; exit 1; }
[[ -f "$FORMULA" ]] || { echo "ERROR: $FORMULA not found" >&2; exit 1; }

echo "==> Homebrew tap bootstrap"
echo "    user      : $GITHUB_USER"
echo "    tap       : homebrew-${TAP_NAME}"
echo "    directory : $TAP_DIR"
echo "    dry run   : $([[ $DRY_RUN -eq 1 ]] && echo yes || echo no)"
echo

if [[ $DRY_RUN -eq 1 ]]; then
    if [[ -e "$TAP_DIR" ]]; then
        echo "[dry-run] $TAP_DIR already exists; no files would be changed"
    else
        echo "[dry-run] would create $TAP_DIR/{Casks,Formula,README.md} and git init"
    fi
    exit 0
fi

if [[ -e "$TAP_DIR" ]]; then
    echo "ERROR: $TAP_DIR already exists. Remove it or choose another --tap-name." >&2
    exit 1
fi

mkdir -p "$TAP_DIR/Casks" "$TAP_DIR/Formula"
cp "$CASK"    "$TAP_DIR/Casks/copypaste.rb"
cp "$FORMULA" "$TAP_DIR/Formula/copypaste-cli.rb"

# Homebrew requires the tap repository to be named homebrew-<name>; the layout
# below (Casks/ and Formula/ at the root) is what `brew tap` expects to find.
cat > "$TAP_DIR/README.md" <<EOF
# homebrew-${TAP_NAME}

Homebrew tap for [CopyPaste](https://github.com/${GITHUB_USER}/copypaste), an
encrypted clipboard manager for macOS and Android.

## Install

\`\`\`sh
brew tap ${GITHUB_USER}/${TAP_NAME}
brew install --cask copypaste      # the app
brew install copypaste-cli         # the command-line client and daemon
\`\`\`

## Why this is not in homebrew/cask

CopyPaste is ad-hoc signed and not notarised — the project has no Apple
Developer ID. homebrew/cask's audit requires notarisation and removes casks
that fail it, so the app is distributed from here instead.

On install, the cask strips the \`com.apple.quarantine\` attribute from
\`/Applications/CopyPaste.app\` — and only from that bundle — and re-signs the
app with a certificate generated on your own machine, which never leaves it.
Homebrew's \`--no-quarantine\` flag was deprecated in Homebrew 5.1 with no
replacement, and Homebrew's own guidance is that post-processing is now
required for software distributed this way.

The app requires no Accessibility or Input Monitoring permission.

## Updating

These files are published by the release workflow in the main repository.
Edit them there, not here.
EOF

cat > "$TAP_DIR/.gitignore" <<'EOF'
.DS_Store
EOF

git -C "$TAP_DIR" init -q -b main
git -C "$TAP_DIR" add -A
git -C "$TAP_DIR" -c user.name="setup-tap.sh" \
                  -c user.email="setup-tap@localhost" \
                  commit -q -m "Initial tap: CopyPaste cask and CLI formula"

echo "    created $TAP_DIR"
echo
cat <<EOF
Next steps — all of them yours to run, none of them automated:

  1. Create an empty repository on GitHub:
       ${GITHUB_USER}/homebrew-${TAP_NAME}

  2. Push it:
       cd "$TAP_DIR"
       git remote add origin git@github.com:${GITHUB_USER}/homebrew-${TAP_NAME}.git
       git push -u origin main

  3. Give the release workflow write access to it. The workflow needs a token
     with contents:write on the TAP repository only — it needs nothing at all
     on this one beyond the default release permission. Store it as the
     HOMEBREW_TAP_TOKEN secret in ${GITHUB_USER}/copypaste.

     This is the one credential in the whole pipeline, and it signs nothing:
     ADR-0001's premise is that CI holds no certificate. If you would rather
     hold no token either, the release workflow attaches the generated cask
     and formula to the GitHub Release as artefacts, and you can copy them
     into the tap by hand.

  4. Verify:
       brew tap ${GITHUB_USER}/${TAP_NAME}
       brew install --cask copypaste
EOF
