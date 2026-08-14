# Pre-push gates

Two CI jobs can only run on hardware GitHub gives us and we do not otherwise
drive: `Windows workspace + installed product evidence (x64)` and
`macOS check + platform (arm64)`. Both went red on `main` on 2026-08-14, and
both were reproducible locally in minutes once somebody looked. These scripts
are that wiring.

## `windows-evidence.ps1` — the Windows job, here

```powershell
scripts/prepush/windows-evidence.ps1                       # the whole job
scripts/prepush/windows-evidence.ps1 -Stage installer,evidence
scripts/prepush/windows-evidence.ps1 -ShowCacheKey         # why would it rebuild?
scripts/prepush/windows-evidence.ps1 -ListStages
```

Stages, in `ci.yml` order: `perl`, `clippy`, `test`, `pydeps`, `frontend`,
`dpapi`, `credmgr`, `fixtures`, `installer`, `evidence`. Every stage runs even
after an earlier one fails, except that `installer` and `evidence` are skipped
when `clippy`, `test` or `frontend` did not pass — there is no point spending
the build on a tree that does not compile. Exit is 0 only when every selected
stage passed.

`evidence` installs the package, drives the installed app through UI Automation
and writes to the real system clipboard. It needs an interactive desktop
session and it will disturb your clipboard, exactly as it does on the runner.

### The installer cache

The installer build is the whole cost of this gate: measured on 2026-08-15 it
was 794 s the first time `target/release` was populated and 299 s after, while
every other stage together came to 160 s. With the cache hit the same run is
134 s. The built package is cached under
`%LOCALAPPDATA%\copypaste-prepush\windows-installer\<key>\`, five entries deep,
and reused whenever the key matches.

The key is a SHA-256 over the git blob id of every input the NSIS bundle is
built from — content, not timestamps, so switching branches back and forth does
not invalidate it, and line endings are normalised the way git normalises them,
because the Tauri build rewrites `src-tauri/Cargo.toml` from CRLF to LF and a
raw-byte key would be invalidated by its own build:

| Input | |
|---|---|
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` | |
| `crates/**` | tracked and untracked, `.gitignore` respected |
| `design/dist/**` | `src/index.css` imports it, so it reaches the bundle |
| `scripts/release/build-windows.ps1`, `package-windows.ps1` | |
| `rustc -vV` and `node --version` | a toolchain change is a different binary |

Nothing else invalidates it: docs, `.github/`, `e2e/`, `tools/` and the other
scripts cannot reach the bundle. The hash covers whole files, so a change
inside a `#[cfg(test)]` block invalidates the cache even though the release
profile never compiles it. That is deliberately conservative.

The toolchain is resolved, not assumed: `1.96` first, then the exact release
`1.96` reports, rejected unless the rustc version matches. A rustup toolchain
can lose `bin/cargo.exe` while its component manifest still lists cargo, and
rustup will then refuse both `component add` and `component remove`.

`-ShowCacheKey` prints the key and, on a miss, names the inputs that differ
from the newest cached entry. `-NoCache` builds regardless.

## `macos-tests.ps1` — the macOS job, on the paired Mac

```powershell
scripts/prepush/macos-tests.ps1
scripts/prepush/macos-tests.ps1 -Ref HEAD~1 -CargoArguments 'clippy --workspace --all-targets --all-features --locked -- -D warnings'
```

Nothing is pushed. A read-only `git daemon` is started on the LAN interface the
Mac reaches, the commit under test is published under a temporary
`refs/prepush/<id>`, and the Mac fetches it into a scratch checkout at
`/Users/dmytro/orca/prepush/copypaste`. That checkout keeps its own `target/`,
so the second run is incremental. The temporary ref and the daemon are removed
in a `finally`, and the daemon is stopped as soon as the Mac's checkout line
proves the fetch landed.

It tests `$Ref` **as committed**. Uncommitted work is reported and left behind,
which is the right semantics for a pre-push gate and the wrong one if you
expected it to test your working tree.

While it is up the daemon serves every ref in the shared repository, read-only,
to one interface. That is the tradeoff taken to avoid pushing a scratch branch.

Two constraints are baked in and will bite anyone extending it:

- The Orca CLI parses `--`-prefixed tokens out of the middle of `--text`, so a
  command is sent base64-encoded.
- The remote tty discards input past its canonical line limit; a ~960-byte
  single line arrived clipped as `... | base6`. The payload is sent in
  256-character appends.

## `wsl/` — the Linux gate scripts

`verify.sh`, `verify2.sh`, `verify3.sh` and `verify-android.sh` used to open
with `cd "$HOME/copypaste"`, which is the coordinator's verification checkout.
A worker that ran one gated somebody else's tree and reported the result for
its own branch. They now take the tree as an argument or in `COPYPASTE_TREE`
and exit 2 with an explanation when neither is given. Each prints the tree,
branch, HEAD, dirty count and effective `CARGO_TARGET_DIR` before it starts.

```sh
~/env-setup/verify.sh ~/sandbox/mytask/copypaste
COPYPASTE_TREE=~/sandbox/mytask/copypaste ~/env-setup/verify3.sh
```

These run from `~/env-setup/` in WSL, outside the repository. The copies here
are the source of truth:

```sh
scripts/prepush/wsl/install.sh            # copy into ~/env-setup
scripts/prepush/wsl/install.sh --check    # report drift, exit 1 if any
```
