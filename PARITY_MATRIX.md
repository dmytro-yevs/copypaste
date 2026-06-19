# CopyPaste — Cross-Platform Parity Matrix

Date: 2026-06-19 · Tracking: CopyPaste-t5qm · Companion docs: `PLATFORM_GAP_REPORT.md` (PG-# gaps), `PARITY_FIX_SUMMARY.md`.

**Columns:** Feature · SoT (source of truth) · daemon · IPC method · CLI · macOS-UI · Android · transport · expected · macOS-actual · Android-actual · CLI-actual · gap (PG-#) · sev · owner · tests · status.
**Legend:** ✓ supported · ✗ absent · ◑ partial · n/a not-applicable · SoT: core=`copypaste-core`, daemon=`copypaste-daemon`, ipc=`copypaste-ipc`, ui-local=client-only.
**Architecture note:** Android speaks UniFFI in-process (no daemon, no socket). Its "support" = functional equivalence via FFI + `ClipboardRepository`. This is the root cause of most drift (Android re-implements daemon-owned logic).

---

## Area 1 — Clipboard History & Content-Types

| Feature | SoT | daemon | IPC | CLI | macOS | Android | expected | macOS-actual | Android-actual | gap | sev | status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| List/paginate | daemon | ✓ `history_page` | `history_page`/`list` | ✓(`list`) | ✓(`history_page`) | ◑ local repo | all via IPC | pinned-first, preview, kind | off-daemon, PAGE 50 | PG-63 | P2/P3 | open |
| Search FTS5 | daemon | ✓ FTS5 | `search` | ✓ | ✓ | ◑ local O(N) decrypt | shared index | FTS5 | re-impl, no FTS | PG-17 | P2 | open |
| Copy-back | daemon | ✓ `copy_item` | `copy_item`/`copy` | ✓ | ✓ | ◑ local write | daemon decrypt+bump+sync | correct | local, bump not synced | PG-18 | P2 | open |
| Delete (tombstone) | daemon | ✓ soft-delete+broadcast | `delete_item`/`delete` | ✓ | ✓ | ✓ local+sync | LWW tombstone | correct | correct | — | — | ok |
| Clear all | daemon | ✓ `delete_all` | `delete_all` | ✓`--force` | ✓ | ✗ UI action | expose clear-non-pinned | ✓ | only `clearUnpinned` wired | PG-46 | P2 | open |
| Pin/unpin | daemon | ✓ `pin_item` | `pin_item` | ✓ | ✓ | ✓ | sync pin | correct | correct | — | — | ok |
| Reorder pinned | daemon | ✓ `reorder_pinned` | `reorder_pinned` | ✗ | ✓ drag | ✓ buttons | sync order | ✓ | ✓ | — | P3 | ok |
| Content-type classify | core `text_kind.rs` | ✓ `classify_text` | `kind` field | ✗ | ✓ daemon kind | ◑ re-impl `TextKind.kt` | shared classifier | daemon | local copy, signal drift | PG-16 | P2 | open |
| Chip colors | ui | n/a | n/a | n/a | table §6 | `chipColorFor` | match | TEXT=blue | TEXT=grey | PG-49 | P3 | open |
| Chip icons | ui | n/a | n/a | n/a | lucide | Compose icons | match | Type/FolderOpen | ContentCopy/AttachFile | PG-50 | P3 | open |
| Span masking | daemon | ✓ `sensitive_spans` | `history_page` | ✗ | ✓ `applySpanMasking` | ✗ no spans | partial-mask | ✓ | secret shown plaintext | PG-4 | **P1** | open |
| Timestamps | daemon `wall_time` | ✓ | all | UTC | absolute locale | relative | consistent | absolute | relative | PG-51 | P3 | open |
| Ordering | daemon | ✓ pinned+lamport DESC | `history_page` | ◑ lamport only | ✓ | ✗ wallTime sort | lamport order | correct | wallTime → drift | PG-19 | P2 | open |
| Large-item badge | daemon `too_large_to_sync` | ✓ | ✓ | ◑ field | ✓ | ✓ local | show badge | ✓ | ✓ | — | — | ok |
| Duplicate handling | daemon | ✓ hash dedup | n/a | n/a | ✓ expectClip | ✓ expectClip | no dup on copy-back | ✓ | ✓ | — | — | ok |
| Empty/error/loading | ui | n/a | n/a | ✓ | ✓ | ✓ | present | ✓ | ✓ | — | — | ok |
| Display limit | daemon MAX=1000 | ✓ | `history_page` | 50 | 200+scroll (not persisted) | 50+scroll | persist limit | not persisted | persisted | PG-32/2b1g | P2 | open |

## Area 2 — Sensitive & Private

| Feature | SoT | daemon | macOS | Android | expected | macOS-actual | Android-actual | gap | sev | status |
|---|---|---|---|---|---|---|---|---|---|---|
| Detector patterns | core | ✓ ≥0.70 | n/a | ✓ FFI | identical | same 37-regex | same (FFI) | — | — | ok |
| `sensitive_kind` vs `is_sensitive` | core | n/a | n/a | ◑ FFI | consistent threshold | n/a | any-conf vs ≥0.70 mismatch | PG-23 | P2 | open |
| `is_sensitive_app` (pw-manager) | core | ✗ unwired | ✗ | ✗ | auto-mark by source | dead code | dead code | PG-22 | P2 | open |
| Store sensitive at capture | daemon/Android | ✓ store+mark+TTL | ✓ | ✗ drops | store both | stored | dropped (`Service.kt:892`) | PG-3 | **P1** | open |
| Store sensitive on sync-in | sync_orch/repo | ✓ re-derive | ✓ | ✓ recompute | both store | ✓ | ✓ (`Repo.kt:1417`) | — | — | ok |
| Exclude sensitive from upload | daemon | ✗ no filter | ✗ uploads | n/a (dropped) | never upload | uploads (jbao) | n/a | PG-15/jbao | **P1** | open |
| Private-mode toggle+persist | daemon/Android | ✓ | ✓ | ✓ | both | ✓ | ✓ | — | — | ok |
| Private-mode enforce (skip) | daemon/Android | ✓ | ✓ | ✓ | suppress capture+sync | ✓ | ✓ | — | — | ok |
| Private-mode degraded boot | daemon | ◑ | ✗ resets false | n/a | restore | resets to false | n/a | PG-14 | **P1** | open |
| Sensitive TTL config | core | ✓ hot-reload | ✓ slider | ✓ slider | same default | 30s | 30s | — | — | ok |
| Per-item `expires_at` | daemon | ✓ stamp+sweep | ✓ | ✗ pruneByAge only | deadline survives suspend | ✓ | survives past TTL | PG-24 | P2 | open |
| Sensitive chip/label | ui | n/a | italic+icon | "PRIVATE" chip | consistent | no text chip | text chip | PG-53 | P3 | open |
| Blur + tap-reveal | ui | `is_sensitive` | ✓ blur6px | ✓ blur5dp/bullets | blur+reveal | ✓ | ✓ (API<31 bullets) | PG-61 | P3 | ok |
| Tap-after-reveal | ui | n/a | auto-copy | toast | consistent | copy | no copy | PG-54 | P3 | open |
| Screenshot guard | ui/os | n/a | ✗ none | ✓ FLAG_SECURE | protect history | none | forced on | PG-25 | P2 | open |
| Export sensitive filter | daemon | ✗ no filter | ✗ plaintext | n/a | exclude/warn | unredacted | n/a | PG-5 | **P1** | open |
| Import re-mark sensitive | daemon | ✗ trusts field | ✗ | n/a | recompute | trusts caller | n/a | PG-26 | P2 | open |
| FTS cleanup on wipe | daemon | ✓ atomic | ✓ | n/a (no FTS) | no orphan plaintext | ✓ | n/a | — | — | ok |
| Logs no plaintext | both | ✓ | ✓ | ✓ length-only | no secrets | ✓ | ✓ | — | — | ok |

## Area 3 — Pairing / QR / SAS / Identity

| Feature | SoT | macOS | Android | expected | macOS-actual | Android-actual | gap | sev | status |
|---|---|---|---|---|---|---|---|---|---|
| QR payload (CPPAIR2) | core | ✓ | ✓ FFI | same format | ✓ | ✓ | — | — | ok |
| QR raw mode | cli | n/a | n/a | CLI-only | ✓ | n/a | — | — | ok |
| QR blur at rest | ui | ✓ blurred | ✓ blurred | blurred default | ✓ | ✓ | — | — | ok |
| QR blur across regen | ui | ✓ preserved | ◑ PairActivity resets | persist | preserved | re-blurs (`:427`) | PG-8 | **P1** | open |
| QR TTL 120s | daemon | ✓ | ✓ | same | 120s | 120s | — | — | ok |
| ≤20s warning | ui | ✓ | ✓ | both | ✓ | ✓ | — | — | ok |
| QR auto-refresh margin | ui | ✓ ≤15s | ✗ at 0s | no dead QR | refresh early | shows expired | PG-56 | P3 | open |
| SAS generation+display | core | ✓ | ✓ | 6-digit | ✓ | ✓ | — | — | ok |
| SAS both-sides confirm | pairing_sm | ✓ | ✓ | explicit, no auto | ✓ | ✓ | — | — | ok |
| SAS timeout 60s | pairing_sm | ✓ | ✓ | same | ✓ | ✓ | — | — | ok |
| Peer fingerprint (SAS modal) | core | ✓ full 64 | ✗ truncated | full for verify | full | take16+…+8 | PG-47 | P2 | open |
| Own fingerprint | core | ◑ (Devices, see Area4) | ✓ full | full both | absent on card | full | PG-9 | P1 | open |
| Failed/expired/retry | pairing_sm | ✓ | ✓ | retry always | ✓ | ✓ | — | — | ok |
| Camera scan | — | ✗ | ✓ ZXing | platform limit | n/a | ✓ | — | limit | documented |
| FLAG_SECURE on QR | os | n/a | ✓ | protect | n/a | ✓ | — | — | ok |

## Areas 4+5 — Devices & Sync Status

| Feature | SoT | macOS | Android | expected | gap | sev | status |
|---|---|---|---|---|---|---|---|
| Own device name | daemon | ComputerName | `Build.MODEL` | same human name | PG-38 | P2 | open |
| Own fingerprint | daemon | ✗ absent | ✓ full | show both | PG-9 | **P1** | open |
| Own model/OS/version/IP | daemon | ✓ | ✓ local | equivalent | — | — | ok |
| Peer name/model/OS/version | daemon `list_peers` | ✓ | ✓ | same | — | — | ok |
| Peer online (60s window) | daemon | ✓ IPC sink | ✓ mDNS | same threshold | (signal differs) | P2 | open |
| Offline dot color | ui | red | grey | same | PG-37 | P2 | open |
| Peer local IP fallback | ui | ✓ extractIp | ✗ | same | PG-39 | P2 | open |
| Peer last-sync format | ui | absolute | relative | consistent | PG-40 | P2 | open |
| Peer fingerprint in row | daemon | ✗ | ✗ | show truncated | PG-45 | P2 | open |
| Transport chip | daemon | ✓ blue/accent | ✓ teal/blue | same | (token) | P3 | open |
| Revoke + rotate | daemon | ✓ rotate option | ✗ revoke only | both | PG-12 | **P1** | open |
| Discovered empty/no-bport hint | ui | ◑ silent | ✓ hint | explain | PG-43 | P2 | open |
| Badge: offline signal | ui | IPC-unreachable | OS-network | same meaning | PG-10 | **P1** | open |
| Badge: connected recency | ui | 5min gate | count>0 | same | PG-11 | **P1** | open |
| Badge: syncing/error | ui | ✓ | ✓ | same | — | — | ok |
| Badge: count source | ui | live IPC | binary fallback | real count | PG-41 | P2 | open |
| Badge: tooltip metadata | ui | ✓ tooltip | ✗ | parity (bottom sheet) | PG-42 | P2 | open |
| Badge: pulse | ui | ✓ ~2s | ✓ ~2s | same | — | — | ok |
| Cloud-misconfig surface | ui | tooltip-only | badge chip | same | PG-44 | P2 | open |

## Areas 6+7 — Settings & UI

| Setting/Component | SoT | macOS | Android | persistence | gap | sev | status |
|---|---|---|---|---|---|---|---|
| theme system/light/dark | ui-local | ✓ | ✓ | both default dark | (CopyPaste-3e6g) | P3 | tracked |
| palette (10) | ui-local | ✓ | ✓ | local | — | — | ok |
| density default | ui-local | compact | comfortable | local | PG-33 | P2 | open |
| mask-sensitive (tab) | ui-local | General | Display | local | PG-48 | P3 | open |
| sound/notify (tab) | daemon | General | Notifications | daemon vs SharedPrefs | PG-48 | P3 | open |
| sync_enabled master | — | ✗ | ✓ | — | PG-30 | P2 | open |
| lan_visibility | daemon | ✓ | ✗ | — | PG-29 | P2 | open |
| relay URL visibility | daemon | always | mode-gated | — | PG-58 | P3 | open |
| supabase email/password | daemon | ✗ UI | ✓ | Keychain/Keystore | PG-13 | **P1** | open |
| sync passphrase | keychain | ✓ clears | ✓ draft | — | (UX) | P3 | open |
| max sizes / quota / TTL | daemon | ✓ | ✓ | daemon vs SharedPrefs | — | — | ok |
| image quality (tab) | daemon | Storage | Display | — | PG-48 | P3 | open |
| history limit persist | ui-local | ✗ not saved | ✓ | — | PG-32 | P2 | open |
| auto_apply_synced_clip | core(unwired) | ✗ | ✗ | absent from IPC AppConfig | PG-31 | P2 | open |
| show_sensitive_warnings | ui-local | ✗ | ✓ | — | PG-34 | P2 | open |
| private_mode storage | daemon/SharedPrefs | daemon-backed | SharedPrefs | — | PG-35 | P2 | open |
| previewLinesPopup | ui-local | ✓ | ✗ single | — | PG-59 | P3 | open |
| Liquid-Blue `:root` accent | ui | `#3D8BFF` | `#4D8DFF` | `#4D8DFF` | PG-36 | P2 | open |
| toast semantic dot | ui | ✗ | ✓ | — | PG-55 | P3 | open |
| card radius | spec §4 | 12px | 14dp | reconcile | PG-57 | P3 | open |
| glass opacity | spec §A.6 | 0.40 | 0.55 | intentional | — | limit | documented |
| About/Logs nav | spec §C | sidebar | via Settings | spec-accepted | — | — | ok |

## Areas 8+9 — IPC Contract & CLI-vs-UI

| Item | macOS-UI | CLI | Android-FFI | expected | gap | sev | status |
|---|---|---|---|---|---|---|---|
| Envelope/error-codes | branch on code | branch on code | n/a (FFI) | consistent | — | — | ok |
| `ipc_not_ready` handling | only HistoryView | ✓ exit | n/a | all views | PG-7 | **P1** | open |
| `version_mismatch` | dead (bridge drops field) | ✓ | n/a | both | PG-6 | **P1** | open |
| `migration_in_progress` retry | ✗ | ✗ | n/a | backoff | PG-20 | P2 | open |
| `daemon_offline` | ✓ multi-view | ✓ | n/a | both | — | — | ok |
| `rate_limited` | only DevicesView | ✓ | n/a | consistent | (P2) | P2 | open |
| Legacy arms drop error_code | shows string | shows string | n/a | machine code | 8u2b | P2 | tracked |
| METHOD_* constants | string literals | constants | n/a | single SoT | PG-62/x2c6 | P3 | open |
| CLI export/import → UI | ✗ | ✓ | ✗ | UI parity | 85n9 | P2 | tracked |
| CLI private → UI | ✓ | ✓ | ✓ | parity | — | — | ok |
| CLI watch/backup/restore/daemon | n/a | ✓ | ✗ | CLI-only by design | — | limit | documented |

## Areas 10+11+12 — Sync Transports

| Transport·Dimension | macOS-daemon | Android | gap | sev | status |
|---|---|---|---|---|---|
| P2P setup/keyderiv/verify | ✓ (shared `copypaste-p2p`) | ✓ same crate | — | — | ok |
| P2P upload/download | persistent link, live fanout | one-shot dial, bounded window | (lifecycle) | P2 | open |
| P2P LWW merge | Rust `remote_wins` | Kotlin-side | risk divergence | PG-? / lcmq | P1 | tracked |
| P2P delete/tombstone | Rust applies | Kotlin must apply | resurrect if omitted | lcmq/0qpn | P1 | tracked |
| P2P inbound listener | always-on | FGS-bound | documented | — | limit | documented |
| P2P inbound Control (Unpair) | handled | dropped (`p2p_listener.rs:324`) | PG-1 | **P1** | open |
| P2P STUN public IP | ✓ | ✗ None | PG-28 | P2 | open |
| Relay PoP registration | ✓ | ✗ no FFI | PG-2 | **P1** | open |
| Relay tombstone/LWW on receive | ✓ | Kotlin (unverified) | risk | (PG-1 family) | P1 | open |
| Relay poll latency | 5s burst | 60s/15min | PG-27 | P2 | open |
| Relay sensitive exclusion | ✗ uploads | drops at capture | PG-15/jbao | P1 | open |
| Cloud WebSocket realtime | ✓ <1s | ✗ poll only | PG-27 | P2 | open |
| Cloud delete/tombstone | ✓ | WS catch-up loss | vfai | P1 | tracked |
| Cloud opt-in/feature-gate | ✓ | ✓ | — | — | ok |
| Inbound sensitive re-detect | ✓ `sync_orch:679` | ✓ `Repo.kt:1417` | — | — | ok |

---

## Summary counts

| Area | OK (parity) | Gaps |
|---|---|---|
| 1 History | 7 | PG-4(P1),16,17,18,19,32,46,49,50,51,63 |
| 2 Sensitive/Private | 8 | PG-3(P1),5(P1),14(P1),15(P1),22,23,24,25,26,53,54 |
| 3 Pairing/QR/SAS | 10 | PG-8(P1),9(P1),47,56 |
| 4/5 Devices/Status | 6 | PG-9(P1),10(P1),11(P1),12(P1),37–45 |
| 6/7 Settings/UI | 9 | PG-13(P1),29–36,48,55,57–59 |
| 8/9 IPC/CLI | 5 | PG-6(P1),7(P1),20,62 |
| 10/11/12 Transports | 7 | PG-1(P1),2(P1),15(P1),27,28 + tracked lcmq/0qpn/vfai |

**Totals: P1=15 · P2=28 · P3=18.** Detail + fix/test plans in `PLATFORM_GAP_REPORT.md`.
