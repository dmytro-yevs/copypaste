# Homebrew Cask Distribution

Maintainer guide for publishing CopyPaste via Homebrew Cask.

## Overview

CopyPaste ships a macOS `.app` bundle inside a `.dmg`, signed with an ad-hoc signature (no Apple Developer ID). Homebrew Cask handles installation, strips quarantine attributes, and registers the daemon LaunchAgent.

Two distribution channels exist:

1. **Official `homebrew-cask`** — long-term goal; reviewed by Homebrew maintainers; users run `brew install --cask copypaste`.
2. **Private tap** (`USER/copypaste`) — bleeding-edge releases between official-cask updates; users opt-in via `brew tap`.

The formula source of truth lives in this repo at `Casks/copypaste.rb`. CI mirrors it to the private tap on every tagged release.

## Release pipeline (per version)

After a `v$VERSION` git tag is pushed and the GitHub Release workflow has built the artifacts:

### 1. Fetch checksums from the release

```bash
VERSION=0.2.0-beta.0
curl -L -o /tmp/SHA256SUMS \
  https://github.com/USER/CopyPaste/releases/download/v${VERSION}/SHA256SUMS
SHA256=$(grep 'CopyPaste.dmg$' /tmp/SHA256SUMS | awk '{print $1}')
echo "DMG sha256: ${SHA256}"
```

### 2. Update the cask formula

Use the generator script shipped with W1.4:

```bash
scripts/release/gen-cask.sh "${VERSION}" "${SHA256}"
```

This rewrites `Casks/copypaste.rb`:

- `version "..."` to the new tag
- `sha256 "..."` replacing `:no_check`
- (the `url` interpolates `#{version}`, so no manual edit needed)

### 3. Local smoke test

```bash
ruby -c Casks/copypaste.rb          # syntax
brew style Casks/copypaste.rb       # style/lint (warnings only)
brew install --cask ./Casks/copypaste.rb
open -a CopyPaste
# Verify: app launches, daemon plist loaded, no Gatekeeper prompt
brew uninstall --cask copypaste
brew uninstall --cask --zap copypaste   # also verifies zap paths
```

### 4. Submit / update the cask

#### 4a. Private tap (every release)

```bash
# In the private tap repo (USER/homebrew-copypaste):
cp /path/to/CopyPaste/Casks/copypaste.rb Casks/copypaste.rb
git add Casks/copypaste.rb
git commit -m "copypaste ${VERSION}"
git push
```

Users update via:

```bash
brew update
brew upgrade --cask copypaste
```

#### 4b. Official homebrew-cask (stable releases only)

1. Fork `https://github.com/Homebrew/homebrew-cask`
2. Create branch `add-copypaste` (initial) or `bump-copypaste-${VERSION}` (updates)
3. Copy `Casks/copypaste.rb` to `Casks/c/copypaste.rb` in the fork
   (homebrew-cask shards by first letter under `Casks/c/`)
4. Commit message: `copypaste ${VERSION}` (terse, one line — required by their style)
5. Open PR following [CONTRIBUTING.md](https://github.com/Homebrew/homebrew-cask/blob/master/CONTRIBUTING.md)
6. Address review comments; CI runs `brew audit`, `brew style`, install/uninstall test
7. Once merged, users worldwide can `brew install --cask copypaste`

## User installation instructions

### Stable (after acceptance into homebrew-cask)

```bash
brew install --cask copypaste
```

### Bleeding-edge via private tap

```bash
brew tap USER/copypaste https://github.com/USER/homebrew-copypaste
brew install --cask copypaste
```

### Update

```bash
brew update
brew upgrade --cask copypaste
```

### Uninstall

```bash
brew uninstall --cask copypaste            # removes app + LaunchAgent
brew uninstall --cask --zap copypaste      # also wipes ~/Library state
```

## Formula reference

Key fields in `Casks/copypaste.rb`:

| Field | Purpose |
|-------|---------|
| `version` | Matches the git tag (without leading `v`) |
| `sha256` | DMG hash; `:no_check` only in pre-tag template |
| `url ... verified:` | `verified:` confirms the URL prefix is owned by the project (required by Homebrew when URL uses redirects) |
| `livecheck` | `strategy :github_latest` auto-discovers new versions for `brew livecheck` |
| `depends_on macos:` | `>= :sonoma` — matches our minimum target |
| `app "CopyPaste.app"` | Symlinked into `/Applications` |
| `postflight` | Strips quarantine (`xattr -cr`) and loads LaunchAgent plist |
| `uninstall launchctl:` | Unloads daemon before removing files |
| `zap trash:` | Deep-clean paths for `--zap` flag |
| `caveats` | Shown post-install — explains ad-hoc signing and daemon control |

## Troubleshooting

| Symptom | Cause / Fix |
|---------|-------------|
| `brew audit` fails on `sha256 :no_check` | Expected pre-tag; run `gen-cask.sh` first |
| Gatekeeper still warns | `postflight` `xattr -cr` did not run — check brew log; manually `xattr -cr /Applications/CopyPaste.app` |
| Daemon not running after install | `launchctl load` failed (plist missing) — re-launch app to regenerate plist, or run `launchctl load -w ~/Library/LaunchAgents/com.copypaste.daemon.plist` |
| `brew upgrade` does not detect new version | Verify `livecheck` works: `brew livecheck --cask copypaste` |
| PR rejected upstream for `sha256 :no_check` | Never submit upstream without a real sha256 |

## References

- Homebrew Cask Cookbook: <https://docs.brew.sh/Cask-Cookbook>
- Acceptable Casks: <https://docs.brew.sh/Acceptable-Casks>
- Contributing to homebrew-cask: <https://github.com/Homebrew/homebrew-cask/blob/master/CONTRIBUTING.md>
- W1.4 release scripts: `scripts/release/gen-cask.sh`
