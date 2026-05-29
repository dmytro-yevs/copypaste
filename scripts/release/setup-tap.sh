#!/usr/bin/env bash
# setup-tap.sh — bootstrap a Homebrew tap repository for CopyPaste.
#
# Creates a homebrew-<tap-name> git repository alongside this checkout,
# seeds it with the current Casks/copypaste.rb, generates a README,
# and installs a GitHub Actions workflow that bumps the cask whenever a
# new release tag is published in the main CopyPaste repo.
#
# Usage:
#   scripts/release/setup-tap.sh --github-user <user> [--tap-name <name>] [--dry-run]
#
# Examples:
#   scripts/release/setup-tap.sh --github-user alice
#   scripts/release/setup-tap.sh --github-user alice --tap-name copypaste --dry-run
#
# Notes:
#   - The tap repo is created at ../homebrew-<tap-name> (sibling of this repo).
#   - This script never pushes; the maintainer creates the GitHub repo and
#     pushes manually (instructions are printed at the end).
#   - Owns NEW files only. Does not modify Casks/copypaste.rb or any other
#     release script — that contract is preserved.
set -euo pipefail

TAP_NAME="copypaste"
GITHUB_USER=""
DRY_RUN=0

print_help() {
    cat <<'EOF'
setup-tap.sh — bootstrap a Homebrew tap for CopyPaste

USAGE:
    scripts/release/setup-tap.sh --github-user <user> [options]

OPTIONS:
    --github-user <user>   GitHub username/org that will host the tap (REQUIRED)
    --tap-name <name>      Tap name (default: copypaste -> homebrew-copypaste)
    --dry-run              Print actions without writing files or running git
    -h, --help             Show this help

OUTPUT:
    Creates ../homebrew-<tap-name>/ containing:
      Casks/copypaste.rb           (copied from this repo, untouched)
      README.md                    (install + update instructions)
      .github/workflows/sync.yml   (auto-bumps cask on upstream release)

NEXT STEPS (printed after run):
    1. Create empty repo on GitHub: <user>/homebrew-<tap-name>
    2. cd ../homebrew-<tap-name> && git push -u origin main
    3. brew tap <user>/<tap-name>
    4. brew install <user>/<tap-name>/copypaste
EOF
}

# ---------- arg parsing ----------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --tap-name)
            [[ $# -ge 2 ]] || { echo "ERROR: --tap-name requires a value" >&2; exit 1; }
            TAP_NAME="$2"
            shift 2
            ;;
        --github-user)
            [[ $# -ge 2 ]] || { echo "ERROR: --github-user requires a value" >&2; exit 1; }
            GITHUB_USER="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            print_help
            exit 0
            ;;
        *)
            echo "ERROR: unknown argument: $1" >&2
            echo "Run with --help for usage." >&2
            exit 1
            ;;
    esac
done

if [[ -z "$GITHUB_USER" ]]; then
    echo "ERROR: --github-user is required" >&2
    echo "Run with --help for usage." >&2
    exit 1
fi

# Validate tap-name shape: lowercase alphanumeric + hyphens (homebrew convention).
if [[ ! "$TAP_NAME" =~ ^[a-z0-9][a-z0-9-]*$ ]]; then
    echo "ERROR: --tap-name must be lowercase alphanumeric + hyphens (got: $TAP_NAME)" >&2
    exit 1
fi

# Validate github-user shape: GitHub usernames allow alphanumeric + hyphens (no leading/trailing).
if [[ ! "$GITHUB_USER" =~ ^[a-zA-Z0-9]([a-zA-Z0-9-]{0,38}[a-zA-Z0-9])?$ ]]; then
    echo "ERROR: --github-user has invalid GitHub username shape (got: $GITHUB_USER)" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SRC_CASK="$REPO_ROOT/Casks/copypaste.rb"
TAP_DIR="$(cd "$REPO_ROOT/.." && pwd)/homebrew-$TAP_NAME"

if [[ ! -f "$SRC_CASK" ]]; then
    echo "ERROR: source cask not found at $SRC_CASK" >&2
    echo "       Run this only after Casks/copypaste.rb exists (W1.5)." >&2
    exit 1
fi

# ---------- runner ----------
run() {
    if [[ $DRY_RUN -eq 1 ]]; then
        echo "[dry-run] $*"
    else
        eval "$@"
    fi
}

write_file() {
    local path="$1"
    local content="$2"
    if [[ $DRY_RUN -eq 1 ]]; then
        echo "[dry-run] write $path ($(echo "$content" | wc -l | tr -d ' ') lines)"
    else
        mkdir -p "$(dirname "$path")"
        printf '%s' "$content" > "$path"
        echo "    wrote $path"
    fi
}

# ---------- preflight ----------
echo "==> Homebrew tap bootstrap"
echo "    tap name      : $TAP_NAME"
echo "    github user   : $GITHUB_USER"
echo "    tap directory : $TAP_DIR"
echo "    dry-run       : $([[ $DRY_RUN -eq 1 ]] && echo yes || echo no)"
echo

if [[ -e "$TAP_DIR" ]]; then
    echo "ERROR: target directory already exists: $TAP_DIR" >&2
    echo "       Remove it or pick a different --tap-name." >&2
    exit 1
fi

# ---------- 1. init repo ----------
echo "==> Creating tap repository skeleton"
run "mkdir -p '$TAP_DIR/Casks' '$TAP_DIR/.github/workflows'"
run "cd '$TAP_DIR' && git init -q -b main"

# ---------- 2. copy cask ----------
echo "==> Copying cask"
if [[ $DRY_RUN -eq 1 ]]; then
    echo "[dry-run] cp $SRC_CASK -> $TAP_DIR/Casks/copypaste.rb"
else
    cp "$SRC_CASK" "$TAP_DIR/Casks/copypaste.rb"
    echo "    copied Casks/copypaste.rb"
fi

# ---------- 3. README ----------
echo "==> Generating README"
README_CONTENT="# homebrew-$TAP_NAME

Homebrew tap for [CopyPaste](https://github.com/$GITHUB_USER/CopyPaste) — an
encrypted clipboard manager with end-to-end sync.

## Install

\`\`\`sh
brew tap $GITHUB_USER/$TAP_NAME
brew install $GITHUB_USER/$TAP_NAME/copypaste
\`\`\`

Or in a single command:

\`\`\`sh
brew install $GITHUB_USER/$TAP_NAME/copypaste
\`\`\`

## Upgrade

\`\`\`sh
brew update
brew upgrade $GITHUB_USER/$TAP_NAME/copypaste
\`\`\`

## Uninstall

\`\`\`sh
brew uninstall $GITHUB_USER/$TAP_NAME/copypaste
brew untap $GITHUB_USER/$TAP_NAME
\`\`\`

## How updates land here

This tap is updated automatically by a GitHub Actions workflow
(\`.github/workflows/sync.yml\`) that watches for new release tags
on the upstream [CopyPaste](https://github.com/$GITHUB_USER/CopyPaste)
repository and bumps \`Casks/copypaste.rb\` (\`version\` + \`sha256\`)
accordingly. Each bump opens a commit on \`main\`.

To trigger a sync manually, dispatch the workflow from the Actions tab.

## Notes

- CopyPaste is **ad-hoc signed** (no Apple Developer ID). The cask
  strips the quarantine attribute on install, so Gatekeeper will not
  warn. See the cask's \`caveats\` block for details.
- Requires macOS Sonoma or newer.
- The daemon installs as a LaunchAgent under your user.

## License

This tap repository is provided as-is. CopyPaste itself is licensed
under its upstream terms (see the main repository).
"
write_file "$TAP_DIR/README.md" "$README_CONTENT"

# ---------- 4. GitHub Actions sync workflow ----------
echo "==> Generating sync workflow"
SYNC_WORKFLOW_CONTENT="name: sync-cask

on:
  # Trigger from upstream via 'gh workflow run' or repository_dispatch
  # (the upstream release workflow can fire a 'release-published' event).
  repository_dispatch:
    types: [release-published]
  workflow_dispatch:
    inputs:
      version:
        description: 'Release version (no leading v), e.g. 0.2.0-beta.1'
        required: true
        type: string
  schedule:
    # Hourly fallback so an upstream release is picked up within ~1h even
    # without a dispatch ping. Cheap; cask repo has no heavy workload.
    - cron: '17 * * * *'

permissions:
  contents: write

jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Resolve version
        id: ver
        env:
          GH_TOKEN: \${{ secrets.GITHUB_TOKEN }}
        run: |
          set -euo pipefail
          if [ -n \"\${{ github.event.inputs.version || '' }}\" ]; then
            VERSION='\${{ github.event.inputs.version }}'
          elif [ -n \"\${{ github.event.client_payload.version || '' }}\" ]; then
            VERSION='\${{ github.event.client_payload.version }}'
          else
            # Fallback: query upstream latest release tag.
            TAG=\$(gh api repos/$GITHUB_USER/CopyPaste/releases/latest --jq .tag_name)
            VERSION=\"\${TAG#v}\"
          fi
          echo \"version=\$VERSION\" >> \"\$GITHUB_OUTPUT\"

      - name: Skip if cask already at this version
        id: check
        run: |
          set -euo pipefail
          CURRENT=\$(awk -F'\"' '/^[[:space:]]*version[[:space:]]+\"/ {print \$2; exit}' Casks/copypaste.rb)
          if [ \"\$CURRENT\" = \"\${{ steps.ver.outputs.version }}\" ]; then
            echo \"already at \$CURRENT — nothing to do\"
            echo \"changed=false\" >> \"\$GITHUB_OUTPUT\"
          else
            echo \"changed=true\" >> \"\$GITHUB_OUTPUT\"
          fi

      - name: Download DMG and compute sha256
        if: steps.check.outputs.changed == 'true'
        id: sha
        env:
          GH_TOKEN: \${{ secrets.GITHUB_TOKEN }}
          VERSION: \${{ steps.ver.outputs.version }}
        run: |
          set -euo pipefail
          URL=\"https://github.com/$GITHUB_USER/CopyPaste/releases/download/v\${VERSION}/CopyPaste-vv\${VERSION}-macos-arm64.dmg\"
          curl -fsSL -o /tmp/CopyPaste.dmg \"\$URL\"
          SHA=\$(sha256sum /tmp/CopyPaste.dmg | awk '{print \$1}')
          echo \"sha256=\$SHA\" >> \"\$GITHUB_OUTPUT\"

      - name: Bump cask
        if: steps.check.outputs.changed == 'true'
        env:
          VERSION: \${{ steps.ver.outputs.version }}
          SHA256:  \${{ steps.sha.outputs.sha256 }}
        run: |
          set -euo pipefail
          awk -v ver=\"\$VERSION\" -v sha=\"\$SHA256\" '
              {
                  if (match(\$0, /^([[:space:]]*)version[[:space:]]+\"[^\"]*\"/, m)) {
                      print m[1] \"version \\\"\" ver \"\\\"\"; next
                  }
                  if (match(\$0, /^([[:space:]]*)sha256[[:space:]]+[^[:space:]]+.*\$/, m)) {
                      print m[1] \"sha256 \\\"\" sha \"\\\"\"; next
                  }
                  print
              }
          ' Casks/copypaste.rb > Casks/copypaste.rb.new
          mv Casks/copypaste.rb.new Casks/copypaste.rb

      - name: Commit and push
        if: steps.check.outputs.changed == 'true'
        env:
          VERSION: \${{ steps.ver.outputs.version }}
        run: |
          set -euo pipefail
          git config user.name  'github-actions[bot]'
          git config user.email 'github-actions[bot]@users.noreply.github.com'
          git add Casks/copypaste.rb
          git commit -m \"chore(cask): bump to \$VERSION\"
          git push
"
write_file "$TAP_DIR/.github/workflows/sync.yml" "$SYNC_WORKFLOW_CONTENT"

# ---------- 5. .gitignore ----------
write_file "$TAP_DIR/.gitignore" ".DS_Store
*.swp
*.bak
"

# ---------- 6. initial commit ----------
echo "==> Creating initial commit"
if [[ $DRY_RUN -eq 1 ]]; then
    echo "[dry-run] cd $TAP_DIR && git add . && git commit -m 'chore: bootstrap tap'"
else
    (
        cd "$TAP_DIR"
        git add .
        # Local commit identity fallback so the bootstrap works on a fresh box.
        if ! git config user.email >/dev/null 2>&1; then
            git config user.email "$GITHUB_USER@users.noreply.github.com"
        fi
        if ! git config user.name  >/dev/null 2>&1; then
            git config user.name  "$GITHUB_USER"
        fi
        git commit -q -m "chore: bootstrap homebrew-$TAP_NAME tap

- Casks/copypaste.rb seeded from upstream CopyPaste@main
- README with install/upgrade/uninstall instructions
- .github/workflows/sync.yml: auto-bump on upstream release tag"
    )
    echo "    committed bootstrap to $TAP_DIR"
fi

# ---------- 7. summary ----------
echo
echo "==> Done."
echo
echo "Next steps:"
echo "  1. Create empty GitHub repo:  $GITHUB_USER/homebrew-$TAP_NAME"
echo "       gh repo create $GITHUB_USER/homebrew-$TAP_NAME --public --source '$TAP_DIR' --remote origin"
echo "     (or via the web UI; do NOT initialize with README — the bootstrap already wrote one)"
echo
echo "  2. Push:"
echo "       cd '$TAP_DIR' && git push -u origin main"
echo
echo "  3. Try the tap:"
echo "       brew tap $GITHUB_USER/$TAP_NAME"
echo "       brew install $GITHUB_USER/$TAP_NAME/copypaste"
echo
echo "  4. (Optional) Wire upstream CopyPaste release workflow to dispatch"
echo "     'release-published' on this repo so the cask bump is instant"
echo "     rather than waiting for the hourly cron. Example:"
echo
echo "       gh api -X POST repos/$GITHUB_USER/homebrew-$TAP_NAME/dispatches \\"
echo "         -f event_type=release-published \\"
echo "         -f 'client_payload[version]=\${VERSION}'"
echo
echo "See docs/release/brew-tap-setup.md for the full guide."
