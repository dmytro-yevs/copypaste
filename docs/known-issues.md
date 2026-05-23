# Known Issues — v0.1.0-alpha.1

## Active limitations
- P2P pairing UI is preview only (`get_own_fingerprint` returns real cert FP, but `pair_peer` / `unpair_peer` are stubs)
- Windows daemon does not run yet — IPC stubs in place for future work
- No code signing — macOS will Gatekeeper-warn on first launch (right-click → Open)
- Android app shell exists; no signed APK
- Relay loses state on restart

## Documented workarounds
- macOS: `xattr -d com.apple.quarantine CopyPaste.app` then double-click
- If daemon crashes: `tail -f ~/Library/Logs/copypaste/daemon.log`
