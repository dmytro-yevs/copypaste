# Branch Inventory — 2026-05-23

**Initial unmerged count:** 9 unmerged `feature/*` branches (+ 3 already-merged `*-complete` branches still around)
**Release HEAD:** `a8436f6` (after ADR-005 merge by this audit)
**Worker:** worker-branch-inventory

## Summary

| Action | Count |
|--------|-------|
| Deleted (BANNED) | 0 |
| Deleted (SUPERSEDED, clean) | 4 |
| Deleted (ALREADY-MERGED, clean) | 1 |
| Merged (DOC-ONLY) | 1 |
| REFUSED (dirty worktree) | 5 |
| Deferred (post-alpha) | 1 |

## Actions taken

### Deleted (BANNED)

_None — no `feature/tauri-macos-ui`, `feature/macos-bundle-daemon`, or `feature/linux-daemon` branches present._

### Deleted (SUPERSEDED — equivalent commits already in release)

| Branch | Evidence | Worktree |
|--------|----------|----------|
| `feature/intg-daemon-cli-e2e` | Identical commit subject `test(daemon): integration test spawning daemon + CLI IPC roundtrip` present in release as `afd522b` | `/Users/dmytro/Documents/CopyPaste-intg-daemon-cli-e2e` removed |
| `feature/slint-ui-hotkey` | `crates/copypaste-ui/` (HistoryWindow + Slint integration) fully present in release; this branch's hotkey work was an earlier iteration on a now-superseded base | `/Users/dmytro/Documents/CopyPaste-slint-ui-hotkey` removed |
| `feature/tray-icon-macos` | Release has `crates/copypaste-daemon/src/tray.rs`, `launchd.rs`, `launch/com.copypaste.daemon.plist`, `launch/install.sh` — all tray-icon work landed via different commit chain | `/Users/dmytro/Documents/CopyPaste-tray-icon-macos` removed |
| `feature/ui-daemon-wire` | Release `crates/copypaste-daemon/src/ipc.rs` (38.8K) already implements full UI ↔ daemon wiring (`history_page`, `paste` handlers, etc.) | `/Users/dmytro/Documents/CopyPaste-ui-daemon-wire` removed |

### Deleted (ALREADY-MERGED — 0 ahead, all commits in release)

| Branch | Behind | Cherry-new |
|--------|--------|------------|
| `feature/ui-complete` | 30 commits behind release | 0 new commits |

### Merged (DOC-ONLY)

| Branch | Resulting commit | Notes |
|--------|------------------|-------|
| `feature/adr-005-slint-ui` | `a8436f6 docs(adr): ADR-005 Slint as UI framework (from feature/adr-005-slint-ui)` | Single new file `docs/adr/ADR-005-slint-ui-framework.md` (61 lines). Written directly into release worktree to avoid disturbing the 15 in-progress modifications by other agents. Worktree and branch then removed. |

### REFUSED (worktree had uncommitted changes)

Per the hard rule "NEVER force-delete a branch with uncommitted changes in its worktree", the following branches were left in place. Each branch is classified as SUPERSEDED or ALREADY-MERGED, but cleanup is deferred until the worktree owner commits or discards their working changes.

| Branch | Worktree | Dirty content | Classification |
|--------|----------|---------------|----------------|
| `feature/intg-ui-merge` | `/Users/dmytro/Documents/CopyPaste-intg-ui-merge` | `M Cargo.lock` | SUPERSEDED — `crates/copypaste-ui/{HistoryWindow,SettingsWindow,PairWindow}.slint` and all `src/` files already in release |
| `feature/merge-ui-slint` | `/Users/dmytro/Documents/CopyPaste-merge-ui-slint` | `M Cargo.lock` | SUPERSEDED — earlier iteration of the same UI work, fully superseded by release `crates/copypaste-ui/` |
| `feature/sqlcipher` | `/Users/dmytro/Documents/CopyPaste-sqlcipher` | `?? .claude`, `?? .claude-flow`, `?? .swarm` (untracked symlinks/state) | SUPERSEDED — SQLCipher integration already in release via `crates/copypaste-core/src/storage/db.rs` (verified `cipher_version`, key pragma, etc.); ADR-003 covers the decision |
| `feature/p2p-complete` | `/Users/dmytro/Documents/CopyPaste-p2p-complete` | `M Cargo.lock` | ALREADY-MERGED — 0 ahead, 29 behind release |
| `feature/supabase-complete` | `/Users/dmytro/Documents/CopyPaste-supabase-complete` | `M Cargo.lock` | ALREADY-MERGED — 0 ahead, 33 behind release |

**Recommended follow-up:** orchestrator confirms no live work, then runs:
```bash
for b in intg-ui-merge merge-ui-slint sqlcipher p2p-complete supabase-complete; do
  git worktree remove /Users/dmytro/Documents/CopyPaste-$b --force
  git branch -D feature/$b
done
```

### UNIQUE-WORK — flagged for tech-lead review

_None this round._ The only unique-work branch (`feature/windows-ipc-named-pipe`) is explicitly deferred per task spec (see below).

### Deferred (post-alpha)

| Branch | Worktree | Reason |
|--------|----------|--------|
| `feature/windows-ipc-named-pipe` | `/Users/dmytro/Documents/CopyPaste-windows-ipc-named-pipe` | IPC refactor splits `crates/copypaste-daemon/src/ipc.rs` (38.8K single file in release) into `ipc/{mod,unix,windows}.rs` plus adds `tokio::net::windows::named_pipe` server. Real unique work (no equivalent in release), but the refactor is too risky mid-release-stabilisation. Hold until post-`v0.1.0-alpha`. |

## Verification details

### Files confirmed present in release `a8436f6`

```
docs/adr/ADR-005-slint-ui-framework.md          (newly merged this audit)
crates/copypaste-ui/src/{lib,main,ipc_client,fingerprint,settings,windows}.rs
crates/copypaste-ui/ui/{appui,history_window,SettingsWindow,PairWindow}.slint
crates/copypaste-daemon/src/{ipc,tray,launchd,keychain}.rs
crates/copypaste-core/src/storage/db.rs        (SQLCipher key pragma + cipher_version)
launch/{com.copypaste.daemon.plist,install.sh}
docs/adr/{001..004}.md                          (existing) + 005 (this audit)
```

### Files confirmed NOT present in release (windows-ipc deferred)

```
crates/copypaste-daemon/src/ipc/mod.rs
crates/copypaste-daemon/src/ipc/unix.rs
crates/copypaste-daemon/src/ipc/windows.rs
```

## Final state

| Metric | Before | After |
|--------|--------|-------|
| `feature/*` branches | 12 | 7 |
| Worktrees | 13 | 8 |
| Unmerged `feature/*` | 9 | 5 (4 superseded but worktree-dirty + 1 deferred) |

### Branches remaining

| Branch | Status | Action when worktree clean |
|--------|--------|---------------------------|
| `feature/intg-ui-merge` | SUPERSEDED (REFUSED) | Delete |
| `feature/merge-ui-slint` | SUPERSEDED (REFUSED) | Delete |
| `feature/sqlcipher` | SUPERSEDED (REFUSED) | Delete |
| `feature/p2p-complete` | ALREADY-MERGED (REFUSED) | Delete |
| `feature/supabase-complete` | ALREADY-MERGED (REFUSED) | Delete |
| `feature/windows-ipc-named-pipe` | DEFERRED | Keep until post-alpha |

### Worktrees remaining

```
~/Documents/CopyPaste                                [main]
~/Documents/CopyPaste-release-alpha                  [release/v0.1.0-alpha]
~/Documents/CopyPaste-intg-ui-merge                  [feature/intg-ui-merge]               REFUSED-dirty
~/Documents/CopyPaste-merge-ui-slint                 [feature/merge-ui-slint]              REFUSED-dirty
~/Documents/CopyPaste-p2p-complete                   [feature/p2p-complete]                REFUSED-dirty
~/Documents/CopyPaste-sqlcipher                      [feature/sqlcipher]                   REFUSED-dirty
~/Documents/CopyPaste-supabase-complete              [feature/supabase-complete]           REFUSED-dirty
~/Documents/CopyPaste-windows-ipc-named-pipe         [feature/windows-ipc-named-pipe]      DEFERRED
```

## Notes for orchestrator

1. The release worktree `/Users/dmytro/Documents/CopyPaste-release-alpha` had **15 modified files + untracked `docs/audit/`** at audit time — clearly in-progress work by another agent. The ADR-005 merge was performed as an isolated new-file commit (no existing files touched) so it does not conflict with that in-progress work.
2. The 5 REFUSED branches all have *trivial* dirty content (`Cargo.lock` rebuild artifacts, untracked `.swarm/.claude*` symlinks). Once the worker(s) confirm those changes can be discarded, cleanup is a one-shot `for` loop (see above).
3. `feature/windows-ipc-named-pipe` should be tracked as a Phase 4+ post-alpha task — recommend storing in `goap-plans` namespace.
