# Homebrew tap setup

This guide walks through creating and maintaining a Homebrew tap for CopyPaste
so end users can install with a single command:

```sh
brew install <user>/copypaste/copypaste
```

The bootstrap is done by `scripts/release/setup-tap.sh`. The cask itself
(`Casks/copypaste.rb`) and the per-release helpers (`cut-tag.sh`,
`build-dmg-ci.sh`, `verify-checksum.sh`, `gen-cask.sh`, `install.sh`) are
**not** modified by this flow — they remain the source of truth.

---

## 1. One-time bootstrap

### Prerequisites

- `git` on `$PATH`
- (Optional) `gh` CLI authenticated with the GitHub account that will host
  the tap repo. If you do not use `gh`, you can create the empty repo
  through the GitHub web UI instead.
- This checkout of CopyPaste with `Casks/copypaste.rb` present (it is — that
  file is owned by worktree W1.5 and already merged).

### Run the bootstrap

```sh
# Replace 'alice' with your GitHub username or org.
scripts/release/setup-tap.sh --github-user alice
```

This creates a **sibling** directory next to your CopyPaste checkout:

```
your-projects/
├── CopyPaste/                 # this repo
└── homebrew-copypaste/        # new tap repo (default name)
    ├── Casks/
    │   └── copypaste.rb       # copied verbatim from CopyPaste/Casks/
    ├── .github/
    │   └── workflows/
    │       └── sync.yml       # auto-bumps cask on upstream release tag
    ├── .gitignore
    └── README.md              # install/upgrade/uninstall instructions
```

An initial git commit is created on the `main` branch. Nothing is pushed.

### Options

| Flag | Default | Purpose |
| --- | --- | --- |
| `--github-user <user>` | (required) | GitHub user or org that will host the tap repo |
| `--tap-name <name>` | `copypaste` | Becomes `homebrew-<name>` |
| `--dry-run` | off | Prints every action without writing files or running git |
| `--help`, `-h` | — | Show usage |

A `--dry-run` produces no side effects — handy for previewing on CI or when
auditing the script before running for real.

### Inspect, then create the GitHub repo

```sh
# 1. Look at what was generated
ls -la ../homebrew-copypaste
cat ../homebrew-copypaste/README.md

# 2. Create the empty GitHub repo (one of):
#    via gh CLI:
gh repo create alice/homebrew-copypaste \
    --public \
    --source ../homebrew-copypaste \
    --remote origin

#    or via the web UI: https://github.com/new
#    -> Owner: alice, Repository name: homebrew-copypaste
#    -> Public, no README/license/.gitignore (the bootstrap wrote those)

# 3. Push
cd ../homebrew-copypaste
git push -u origin main
```

The repo **must** be named `homebrew-<tap-name>` exactly — Homebrew's tap
resolver depends on this prefix.

---

## 2. End-user install

After the tap exists on GitHub, anyone can install with:

```sh
brew tap alice/copypaste
brew install alice/copypaste/copypaste
```

Or, skipping the explicit tap step (Homebrew auto-taps on first use):

```sh
brew install alice/copypaste/copypaste
```

### Upgrade

```sh
brew update
brew upgrade alice/copypaste/copypaste
```

### Uninstall

```sh
brew uninstall alice/copypaste/copypaste
brew untap alice/copypaste
```

---

## 3. How updates flow into the tap

The tap repository ships with `.github/workflows/sync.yml`, which keeps
`Casks/copypaste.rb` in sync with new CopyPaste releases. It runs on three
triggers:

1. **`repository_dispatch` event `release-published`** — the recommended
   path. The upstream CopyPaste release workflow fires this event after a
   tag + DMG upload completes; the cask is bumped within seconds.
2. **`workflow_dispatch`** — manual trigger from the Actions tab; useful
   for re-running a sync or bumping to an arbitrary version.
3. **Hourly cron (`17 * * * *`)** — a cheap fallback in case the
   `repository_dispatch` ping is missed. Worst case the cask lags upstream
   by ~1 hour.

For each trigger the workflow:

1. Resolves the target version (input → dispatch payload → latest GitHub
   release tag).
2. Skips if `Casks/copypaste.rb` already has that version.
3. Downloads `https://github.com/<user>/CopyPaste/releases/download/v<ver>/CopyPaste.dmg`.
4. Computes `sha256sum`.
5. Rewrites the `version` and `sha256` lines in the cask (`awk` — same
   shape as `scripts/release/gen-cask.sh`).
6. Commits as `github-actions[bot]` with message
   `chore(cask): bump to <version>` and pushes to `main`.

### Wiring upstream to dispatch the event

In the upstream CopyPaste release workflow (after the DMG is uploaded),
add a step like:

```yaml
- name: Notify tap
  env:
    GH_TOKEN: ${{ secrets.TAP_DISPATCH_PAT }}   # PAT with repo:write on the tap repo
    VERSION: ${{ steps.cut.outputs.version }}    # bare version, no leading 'v'
  run: |
    gh api -X POST repos/alice/homebrew-copypaste/dispatches \
      -f event_type=release-published \
      -f "client_payload[version]=$VERSION"
```

`secrets.GITHUB_TOKEN` cannot dispatch cross-repo, so a fine-grained
personal access token with `contents: write` on the tap repo is required.

### Manual bump (no upstream wiring)

From the tap repo's Actions tab → **sync-cask** → **Run workflow** →
enter the version (no leading `v`). The workflow downloads the DMG,
recomputes the sha256, and pushes the bump.

---

## 4. Verifying the install end-to-end

After a release tag is cut and the tap has been bumped:

```sh
# Clean state
brew uninstall alice/copypaste/copypaste 2>/dev/null || true

# Fresh install from the tap
brew install --verbose alice/copypaste/copypaste

# Sanity
ls /Applications/CopyPaste.app
launchctl list | grep com.copypaste
tail -F ~/Library/Logs/copypaste/daemon.log
```

`brew audit --cask alice/copypaste/copypaste` should be clean (the cask is
copied verbatim from `Casks/copypaste.rb`, which already passes audit).

---

## 5. Troubleshooting

**`Error: Cask 'copypaste' is unavailable`** — the tap is not added. Run
`brew tap alice/copypaste` first, or use the fully-qualified name
`alice/copypaste/copypaste`.

**`Error: SHA256 mismatch`** — the cask points at a version whose DMG has
been re-uploaded with different bytes. Re-run the **sync-cask** workflow
(it recomputes the sha256 from the current artifact) and commit the bump.

**Workflow auth error on cross-repo dispatch** — `secrets.GITHUB_TOKEN`
cannot trigger workflows in another repository. Create a fine-grained PAT
with `contents: write` on `homebrew-copypaste` and store it as
`TAP_DISPATCH_PAT` in the upstream CopyPaste repo.

**Gatekeeper warning despite the cask** — CopyPaste is ad-hoc signed; the
cask's `postflight` strips the quarantine attribute. If the warning still
appears, run:

```sh
xattr -cr /Applications/CopyPaste.app
```

---

## 6. Reference

| File | Owner | Purpose |
| --- | --- | --- |
| `scripts/release/setup-tap.sh` | this task | One-shot bootstrap of the tap repo |
| `docs/release/brew-tap-setup.md` | this task | This guide |
| `Casks/copypaste.rb` | W1.5 (stable) | Source of truth for the cask |
| `scripts/release/gen-cask.sh` | W1.5 | Local bump (`version` + `sha256`) used by maintainer |
| `scripts/release/cut-tag.sh` | release flow | Tag a release |
| `scripts/release/build-dmg-ci.sh` | release flow | Build the DMG in CI |
| `scripts/release/verify-checksum.sh` | release flow | Verify DMG sha256 |
| `scripts/release/install.sh` | release flow | Direct (non-brew) install |
