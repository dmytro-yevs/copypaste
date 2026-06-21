# CopyPaste Cross-Platform Parity Matrix

**Audited:** 2026-06-20 · **Read-only audit; every cell verified against source (`file:line`).**
**Sources of truth:** `docs/PARITY-SPEC.md`, `docs/protocol.md`, `docs/relay-api.md`, `docs/design/DESIGN-SYSTEM-v2.md`.
**Platforms:** `daemon` = copypaste-daemon (runtime truth) · IPC = copypaste-ipc · CLI = copypaste-cli · macOS = crates/copypaste-ui (Tauri/React) · Android = android/ (Compose, standalone UniFFI node — **no daemon, no IPC socket, no P2P**).

**Parity scripts:** `parity-check.mjs` → **PASS (53/53 ±5)** · `check-skin-parity.mjs` → **PASS (21/21 tokens)**.

Legend — status: ✅ parity · ⚠️ partial / drift · ❌ missing · N/A. Severity per PARITY-SPEC rubric (undocumented drift=bug; security/identity/sync-status ≥P1; undocumented-missing-parity ≥P2; visual ≥P2/P3).

---

## 1. Clipboard history

| Feature | Source | daemon | IPC method | CLI | macOS | Android | expected | actual macOS | actual Android | actual CLI | gap | sev | test cov |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| List (paginated, newest-first, pinned-first) | protocol | ✅ | `history_page`/`list` | ✅ | ✅ | ✅ | paginated, pinned first | `historyPage` PAGE_SIZE 200, load-more (HistoryView.tsx:1475) | `loadItems`/`loadMore`, distinctBy id (HistoryActivity.kt:634,2355) | legacy `list` (list.rs:4) | ⚠️ CLI legacy method | P3 | yes |
| Search (FTS) | protocol | ✅ | `search` | ✅ | ✅ | ✅ | FTS over content | FTS+substring, debounce 250ms (HistoryView.tsx:1800) | snippet+full-decrypt union (HistoryActivity.kt:641-685) | single page, no kind col (search.rs:7) | ⚠️ CLI output thin | P3 | yes |
| Copy-back | protocol | ✅ | `copy`/`copy_item` | ✅ | ✅ | ✅ | restore to clipboard | `copyItem` UUID (HistoryView.tsx:1986) | OS clip API + bump IPC (HistoryActivity.kt:2327) | legacy `copy` (copy.rs:148) | ⚠️ legacy/bypass | P3 | yes |
| Delete single | protocol | ✅ | `delete`/`delete_item` | ✅ | ✅ | ✅ | remove by id | `delete_item`+5s undo (HistoryView.tsx:1531) | immediate, NO undo (HistoryActivity.kt:1219) | immediate (delete.rs:4) | ⚠️ Android no undo | **P1** | partial |
| Clear all / unpinned | protocol | ✅ | `delete_all` | ✅ | ✅ | ✅ | clear history | Settings→Storage "Clear history…" (SettingsView.tsx:2596) | clearUnpinned+clearAll (HistoryActivity.kt:268,760) | `clear --force` (clear.rs:23) | ✅ (subagent refuted) | — | yes |
| Pin / unpin | protocol | ✅ | `pin_item` | ✅ | ✅ | ✅ | toggle pin | toggle+bulk (HistoryView.tsx:2129) | setPinned+bulk (HistoryActivity.kt:820) | pin/unpin (pin.rs:45) | ✅ | — | yes |
| Reorder pinned | protocol | ✅ | `reorder_pinned` | ❌ | ✅ drag | ✅ arrows | persist order | drag (HistoryView.tsx:2143) | up/down arrows (HistoryActivity.kt:1221) | none | ⚠️ UX differs | P3 | yes |
| Content-type chips/previews | DESIGN §6 | ✅ | (in item) | text | ✅ | ✅ | kind chip + preview | icon tile+swatch+host split (HistoryView.tsx:655) | icon tile+chip (HistoryActivity.kt:1963) | KIND/TYPE/PREVIEW cols (list.rs) | ⚠️ CLI omits too_large/origin/app | P3 | yes |
| Timestamps / ordering | PARITY §3 | ✅ | (in item) | ✅ | ✅ | ✅ | 11px tabular, newest first | ✅ | ✅ live tick | wall_time col | ✅ | — | yes |
| Dup handling | core | ✅ content_hash | — | n/a | client distinct | distinctBy id | minute-bucket dedup | client dedup on append | client distinctBy (2355) | n/a | ✅ | — | yes |
| Large item | — | ✅ blob_ref | — | n/a | ✅ LRU | ✅ 16+8 MiB LRU | height pref, cache | imageMaxHeight (HistoryView.tsx:254) | LRU caches (HistoryActivity.kt:361) | no images | ✅ | — | yes |
| Empty state | DESIGN | n/a | — | text | ✅ | ✅ | friendly empty | EmptyState (HistoryView.tsx:1496) | EmptyHistoryState (HistoryActivity.kt:1742) | text | ✅ | — | yes |
| Error / degraded-DB recovery | — | ✅ `reset_database` | `reset_database` | n/a | ✅ Reset button | ❌ none | recover path | degraded UI + Reset (HistoryView.tsx:2321) | none (rg→0) | n/a | ⚠️ Android no recovery | P2 | no |
| Sort by device | — | n/a | (client) | ❌ | ✅ toggle | ❌ | group by origin device | sortMode device (HistoryView.tsx:1484,2514) | none (HistoryActivity.kt:622) | none | ⚠️ Android missing | P3 | yes (macOS) |
| Device filter | — | n/a | (client) | ❌ | ✅ dropdown | ✅ chip row | filter by origin | dropdown (HistoryView.tsx:1481) | chip row >1 device (HistoryActivity.kt:551) | none | ✅ (CLI n/a) | — | yes |

## 2. Sensitive / private

| Feature | Source | daemon | IPC method | CLI | macOS | Android | expected | actual macOS | actual Android | actual CLI | gap | sev | test |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Detector | core | ✅ `is_sensitive`/`sensitive_kind` | — | ✅ | ✅ | ✅ (FFI) | shared classifier | from item flag | UDL `is_sensitive`/`sensitive_kind` | `*`/`[sensitive]` marker | ✅ | — | yes |
| Sensitive chip/label | DESIGN §6 | n/a | — | text | ⚠️ content-type chip | ⚠️ "PRIVATE" red | one canonical label | keeps content-type, blurs text (HistoryView.tsx:716) | forces "PRIVATE" danger (HistoryActivity.kt:1987) | yes/no col | ⚠️ inconsistent | P3 | yes |
| Blur / reveal | DESIGN | n/a | — | n/a | ✅ blur(6px)+click | ✅ blur(6dp)+tap | blur at rest | CSS blur (HistoryView.tsx:716-757) | Modifier.blur API31+, bullet-mask <31 (HistoryActivity.kt:2651) | n/a | ✅ | — | yes |
| Private mode (pause) | protocol | ✅ | `set/get_private_mode` | ✅ | ✅ (Settings) | ✅ (Settings) | toggle recording | SettingsView | SettingsActivity.kt:514,840 | private.rs:9 | ✅ | — | yes |
| TTL expiry / retention | core | ✅ daemon loop | (config) | n/a | n/a (daemon) | ✅ local loop | prune general+sensitive | daemon-enforced | ClipboardRepository.kt:1400-1462 | n/a | ✅ split | — | yes |
| No-sync for sensitive | PARITY P1-1 | ✅ | — | n/a | ✅ | ✅ | never upload sensitive | relay+cloud skip | core skips; relay/cloud skip | n/a | ✅ (relay refuted) | — | yes (relay.rs:2229) |
| FTS indexing of sensitive | core | indexes all | `search` | indexes | indexes | indexes (decrypt) | consistent | indexed (items.rs:446) | indexed | indexed | ✅ consistent (refuted P1) | — | yes |
| Logs/UI don't expose secrets | security | ✅ scrub | — | masks | blur | blur/mask | no plaintext leak | blur+reveal gated | blur+reveal gated | masks | ✅ | — | partial |
| Import/export/backup | protocol | ✅ | `export`/`import` | ✅ | ✅ Settings | ❌ | GUI backup | Settings export/import (SettingsView.tsx:1432+) | NONE; not in UDL | export.rs/import.rs | ❌ Android missing | **P1** | yes (CLI/macOS) |
| Bulk copy excludes sensitive | — | n/a | — | n/a | ✅ | ✅ | filter sensitive | filter (HistoryView.tsx:2283) | filter (HistoryActivity.kt:846) | n/a | ✅ | — | yes |

## 3. Pairing / QR

| Feature | Source | daemon | IPC method | CLI | macOS | Android | expected | actual macOS | actual Android | gap | sev | test |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| QR generation | protocol | ✅ | `pair_generate_qr` | ✅ | ✅ | ✅ | short-lived payload | auto+refresh 15s pre-exp (DevicesView.tsx:843) | auto on compose (PairActivity.kt:858; DevicesActivity OwnQrSection:1014) | ✅ | — | yes |
| Raw mode | — | ✅ | — | ✅ `--raw` | n/a | n/a | bare payload+warning | n/a | n/a | ✅ (CLI only) | — | yes |
| Blur + reveal on QR (every visual platform) | PARITY §10 | n/a | — | n/a (terminal) | ✅ default-blurred | ✅ default-blurred | blur at rest, tap reveal | QrBlur default blurred (DevicesView.tsx:755,1469) | qrRevealed=false / qrBlurred=true (PairActivity.kt:410; DevicesActivity.kt:1229) | ✅ | — | yes |
| Regen preserves blur | PARITY §10 (v5a) | n/a | — | n/a | ✅ | ✅ | regen keeps blur | handleQrRegenerate no reset (DevicesView.tsx:789) | generateQr no mutate (PairActivity.kt:431; DevicesActivity.kt:1263) | ✅ | — | yes |
| Countdown + ≤20s warning | PARITY §10 | n/a | — | ✅ text | ✅ accent→warning ≤20s | ✅ ≤20s | warning at ≤20s | bar+text warning ≤20 (DevicesView.tsx:1514,1523) | URGENT=20 both screens (PairActivity.kt:116; DevicesActivity.kt:1179) | ✅ | — | yes |
| Countdown drain-bar | PARITY §10 | n/a | — | n/a | ✅ | ⚠️ DevicesActivity only | bar on QR screens | drain bar | bar in DevicesActivity (1455); PairActivity text-only (1176) | ⚠️ inconsistent across Android screens | P3 | partial |
| SAS display + confirm | protocol | ✅ | `pair_get_sas`/`pair_confirm_sas` | ❌ | ✅ glass modal 28px | ✅ GlassAlertDialog 28sp | SAS digits + confirm | full overlay, tap-copy (DevicesView.tsx:354) | cells 28sp tap-copy (DevicesActivity.kt:2329) | ✅ (struct differs) | P3 (visual) | yes |
| SAS peer-metadata card | — | ✅ snapshot | — | n/a | ✅ name/IP/fp card | ❌ | corroborate identity | surface-card (DevicesView.tsx:363-375) | none (DevicesActivity.kt:2306-2346) | ⚠️ Android missing | P2 | no |
| Fingerprint display | security | ✅ | — | n/a | ✅ own+peer | ✅ own+peer | show fingerprint | FingerprintRow first8…last8 (DeviceCard.tsx:124-200) | own full, peer first16…last8 (DevicesActivity.kt:1642,1923) | ✅ | — | yes |
| Failed/expired/timeout state | protocol | ✅ states | `pair_get_sas` | n/a | ✅ all 4 | ✅ all 4 | confirmed/rejected/aborted/timed_out | DevicesView.tsx:405-464 | DevicesActivity.kt:2288-2411 | ✅ | — | yes |
| Retry after terminal | — | ✅ auto-reset | `pair_abort` | n/a | ✅ | ✅ | re-pair after fail | trailing-idle reset (DevicesView.tsx:184) | mirror (DevicesActivity.kt:2183); pairing_sm.rs:268-280 | ✅ | — | yes |
| Camera/scan limits + FLAG_SECURE | security | n/a | — | n/a | n/a | ✅ FLAG_SECURE | block screenshots of QR | n/a (desktop) | FLAG_SECURE (PairActivity.kt:185; DevicesActivity.kt:367) | ✅ | — | yes |

## 4. Devices screen

| Feature | Source | daemon | CLI | macOS | Android | expected | actual macOS | actual Android | gap | sev |
|---|---|---|---|---|---|---|---|---|---|---|
| Own device card (first) | DESIGN §C | ✅ | ❌ | ✅ | ✅ | own card first | ThisDeviceCard (DeviceCard.tsx:176) | OwnDeviceRow (DevicesActivity.kt:1030) | ✅ | — |
| Peer cards | DESIGN §C | ✅ | ❌ | ✅ | ✅ | per-peer | PeerRow (DeviceCard.tsx:254) | PeerRow (DevicesActivity.kt:1519) | ✅ | — |
| Discovered (mDNS) | protocol | ✅ | ❌ | ✅ | ✅ | list+rescan | poll 3s+rescan btn (DevicesView.tsx:1376) | DiscoveredPeerRow poll 2s (DevicesActivity.kt:1944) | ✅ | — |
| Verified badge | DESIGN | ✅ | ❌ | ✅ | ✅ | trust chip | trust-badge (DeviceCard.tsx:313) | Verified chip (DevicesActivity.kt:1569) | ✅ (refuted) | — |
| Last seen / online-offline | PARITY §9 | ✅ | ❌ | ✅ live | ✅ live | dot + relative time | StatusDot+1s tick (DeviceCard.tsx:295) | PulseDot+1s (DevicesActivity.kt:1535,2448) | ✅ | — |
| Platform / model / OS / version | DESIGN | ✅ | ❌ | ✅ | ✅ | metadata rows | Model/OS/Version (DeviceCard.tsx:329-331) | model/OS/version (DevicesActivity.kt) | ✅ | — |
| Pubkey / fingerprint | security | ✅ | ❌ | ✅ tap-copy | ✅ tap-copy | show + copy | FingerprintRow (DeviceCard.tsx:132) | DevicesActivity.kt:1642 | ✅ | — |
| Transport indicator (P2P/relay/cloud) | — | ✅ | ❌ | ✅ chip | ✅ chip | transport chip | P2P/Cloud (DeviceCard.tsx:279) | TransportChip (DevicesActivity.kt:1560) | ✅ | — |
| Discovered peer IPs | — | ✅ | ❌ | ✅ all IPs | ⚠️ first only | show addresses | joined (DevicesView.tsx:501) | firstOrNull (DevicesActivity.kt:1953) | ⚠️ Android first-only | P2 |
| Remove / unpair / revoke | protocol | ✅ | ❌ | ✅ 2-path | ✅ 2-path | unpair + revoke+rotate | revoke dialog (DevicesView.tsx:548) | GlassAlertDialog (DevicesActivity.kt:688) | ✅ | — |

## 5. Online / sync status

| Feature | Source | daemon | IPC | CLI | macOS | Android | expected | actual macOS | actual Android | gap | sev |
|---|---|---|---|---|---|---|---|---|---|---|---|
| States connected/idle/offline/syncing/error | PARITY §9 | ✅ `badge_state` | `get_sync_status` | ✅ `status` | ✅ 3+pulse | ✅ resolveSyncBadgeState | success/idle/offline | SyncStatus component | SyncStatusBadge.kt:170 resolveSyncBadgeState | ✅ | — |
| Pulse (2s connected) | PARITY §9 | n/a | — | n/a | ✅ | ✅ | pulse animation | ✅ | PulseDot | ✅ | — |
| Tooltip | PARITY §9 | n/a | — | n/a | ✅ | ✅ contentDescription | hover/a11y tooltip | ✅ | statusCd (SyncStatusBadge.kt:209) | ✅ | — |
| Retry/backoff on `ipc_not_ready` | protocol | ✅ | — | ✅ | ✅ | n/a (no IPC) | transient backoff | clients retry | Android has no socket IPC | ✅ (n/a Android) | — |
| Relay/cloud/P2P unavailable display | — | ✅ | `get_sync_status` | ✅ | ✅ | ✅ offline/error | surface unavailability | per-transport status | OFFLINE/ERROR → red dot (SyncStatusBadge.kt:520) | ✅ | — |
| `badge_state` consumed from IPC | protocol | ✅ emits | `get_sync_status` | text | ✅ | ⚠️ mapper present, wired? | use authoritative badge | macOS maps it | `IpcSyncBadgeState`/`toSyncBadgeState` exist (SyncStatusBadge.kt:558-606); call-site uses local heuristic | ⚠️ Android relies on heuristic, not authoritative IPC string | P3 |

## 6. Settings

| Setting | Source | daemon | IPC | CLI | macOS | Android | one source of truth + persisted | gap | sev |
|---|---|---|---|---|---|---|---|---|---|
| Theme System/Light/Dark | PARITY §0 | ✅ config | `set/get_config` | n/a | ✅ segmented | ✅ IdeSegmentedControl | persisted; **default contradicts spec** | ⚠️ both default dark vs spec light | **P1** (doc drift) |
| Palettes | PARITY §A | ✅ | config | n/a | ✅ | ✅ | per-palette tokens | ✅ scripts pass | — |
| Density (Comfortable/Compact) | PARITY §7 | ✅ | config | n/a | ✅ segmented | ✅ IdeSegmentedControl | segmented both | ✅ | — |
| Private mode | protocol | ✅ | `set_private_mode` | ✅ | ✅ | ✅ | persisted | ✅ | — |
| Sync/relay/cloud/storage | protocol | ✅ | `set_config` | ✅ `cloud` | ✅ Supabase section | ✅ SyncBackend | configurable | ⚠️ Android single-transport | P2 |
| Retention / TTL | core | ✅ | config | n/a | ✅ | ✅ | sliders | ✅ | — |
| Telemetry opt-in/out | telemetry | ✅ | config | n/a | ✅ | ✅ | persisted | ✅ | — |
| Vacuum / stats / DB maintenance | protocol | ✅ | `vacuum`/`stats` | ✅ | ❌ no UI | ❌ | GUI maintenance | ⚠️ no GUI surface | P2 |
| Advanced / debug / Log viewer | DESIGN §C | ✅ logs | — | n/a | ✅ LogView | ✅ LogViewerActivity | log viewer | ✅ | — |

## 7. UI parity (tokens / components / motion)

| Item | Source | macOS | Android | expected | gap | sev |
|---|---|---|---|---|---|---|
| Light-first default | PARITY §0 | ⚠️ dark (App.tsx:283) | ⚠️ dark (Settings.kt:55) | light-first | ⚠️ both dark, spec says light | **P1** |
| Per-palette accent tokens | PARITY §A.3 | ✅ | ✅ | ±5 | ✅ parity-check 53/53 | — |
| Skin canonical tokens | DESIGN | ✅ | ✅ | 21 tokens | ✅ skin-parity 21/21 | — |
| Liquid-blue accent #4D8DFF | PARITY §A.3 | ✅ index.css:120 | ✅ Color.kt:53 | match | ✅ resolved | — |
| IdeFaint #82868F/#6C6C72 | PARITY §1 | ✅ | ✅ Color.kt:42,110 | AA | ✅ resolved | — |
| IdeGhost/IdeGhostDeco | PARITY §A.1 | ✅ | ✅ Color.kt:48-49 | match | ✅ | — |
| Typography (Inter+JBMono, 14px title, grey section) | PARITY §3 | ✅ | ✅ | match | ✅ | — |
| Radii chip4/ctrl6/card12/modal16 | PARITY §4 | ✅ | ✅ | match | ✅ | — |
| Glass cards (all device cards routed) | PARITY §2/§8 | ✅ surface-card | ✅ CopyPasteCard (all) | one glass path | ✅ | — |
| Glass dark-opacity | PARITY §A.6 | .40 (vibrancy) | 0.55f (Components.kt:107) | intentional delta | ⚠️ no rationale comment on Android | P3 |
| Nav active/inactive (dim, no rainbow) | PARITY §9 | ✅ dim (Sidebar.tsx:128) | ✅ c.dim | uniform dim | ✅ | — |
| Chips (9px uppercase, tinted+border) | DESIGN §6 | ✅ | ✅ | match | ✅ | — |
| Switch 34×18, white thumb, no glow | PARITY §7 | ✅ | ✅ Components.kt:1113 | match | ✅ | — |
| Segmented control (density+theme) | PARITY §7 | ✅ | ✅ IdeSegmentedControl (bespoke) | iOS-style | ✅ (M3 SCSBR intentionally avoided) | — |
| Sliders (4px track, no halo) | PARITY §7 | ✅ | ✅ | match | ✅ | — |
| Dialogs (glass, blurred scrim) | PARITY §8 | ✅ | ✅ GlassAlertDialog | glass | ✅ | — |
| Toast (glass, semantic dot, 180ms) | PARITY §8 | ✅ | ✅ GlassToast | glass | ✅ | — |
| Headers (glass, 14px medium) | PARITY §8 | ✅ ViewShell | ✅ CopyPasteTopBar | glass | ✅ | — |
| Disabled opacity 0.40 | PARITY §4 | ✅ | ✅ Components.kt:1153 | 0.40 | ✅ resolved | — |
| Icons (outline) | PARITY §5 | ✅ Lucide | ✅ Icons.Outlined.* (QrCode filled OK) | thin outline | ✅ | — |
| Motion (instant90/fast130/base180/slow240) | PARITY §A.7 | ✅ | ✅ | match | ✅ | — |
| Stale token-legend comments | — | n/a | ⚠️ Color.kt:17, Theme.kt:33 | accurate comments | ⚠️ stale | P3 |

## 8. IPC error_code branching

| Item | Source | daemon | CLI | macOS | Android | expected | actual | gap | sev |
|---|---|---|---|---|---|---|---|---|---|
| Error codes declared | protocol | ✅ `ERR_CODE_*` (protocol.rs) | — | — | n/a | stable codes | not_found/auth_failed/invalid_argument/not_implemented/ipc_not_ready/internal_error/version_mismatch/migration_in_progress/rate_limited | ✅ | — |
| Branch on `error_code` not text | protocol | ✅ emits | ⚠️ CLI uses error text in places | ✅ branches | n/a | prefer error_code | macOS branches on code; CLI partly text | ⚠️ minor CLI | P3 |
| Backward-compat (missing error_code) | protocol | ✅ skip_if_none | ✅ tolerates | ✅ tolerates | n/a | treat absent as unknown | ✅ | — |

## 9. CLI vs UI command parity

| CLI cmd | daemon/IPC | macOS-UI equivalent | Android | gap | sev |
|---|---|---|---|---|---|
| list | `history_page` | ✅ History | ✅ | ✅ | — |
| count | `count` | indirect (total) | indirect | ⚠️ no dedicated UI | P3 |
| status | `status` | ✅ Settings | ✅ badge | ✅ | — |
| stats | `stats` | ❌ | ❌ | ⚠️ no GUI | P2 |
| search | `search` | ✅ | ✅ | ✅ | — |
| copy | `copy_item` | ✅ | ✅ | ✅ | — |
| delete | `delete_item` | ✅ (+undo) | ✅ (no undo) | ⚠️ Android no undo | P1 |
| clear --force | `delete_all` | ✅ Settings | ✅ | ✅ | — |
| pin/unpin | `pin_item` | ✅ | ✅ | ✅ | — |
| private | `set/get_private_mode` | ✅ | ✅ | ✅ | — |
| watch | (none) | ❌ (poll) | ❌ (poll) | ⚠️ no event stream | P3 |
| export/import | `export`/`import` | ✅ Settings | ❌ | ❌ Android | P1 |
| backup/restore | (file-level) | ❌ | ❌ | ⚠️ CLI-only | P2 |
| vacuum | `vacuum` | ❌ | ❌ | ⚠️ no GUI | P2 |
| pair-qr | `pair_generate_qr` | ✅ | ✅ | ✅ | — |
| cloud setup/status/test | config/`cloud_test_connection` | ✅ Supabase | ✅ | ✅ (setup-sql CLI-only) | P3 |
| daemon start/stop/… | (lifecycle) | ⚠️ RestartDaemonButton only | n/a (no daemon) | ⚠️ partial macOS | P3 |

## 10. Sync transport parity (P2P / relay / cloud)

| Aspect | Source | daemon | macOS | Android | expected | gap | sev |
|---|---|---|---|---|---|---|---|
| P2P mTLS + mDNS | p2p.rs | ✅ | ✅ | ❌ (relay/cloud only) | LAN sync | ❌ Android no P2P | P2 |
| P2P key derivation (PAKE→HKDF, SAS verify) | p2p.rs:184 | ✅ | ✅ | n/a | session key | ✅ (macOS) | — |
| Relay register/upload/poll/dedup | relay.rs | ✅ | ✅ | ✅ | LWW on item_id | ✅ | — |
| Relay sensitive exclusion | relay.rs:811-816 | ✅ | ✅ | ✅ | never upload | ✅ (refuted P1) | — |
| Relay + cloud additive | relay.rs:38-59 | ✅ | ✅ | ❌ mutually exclusive | both run | ⚠️ Android single | P2 |
| Relay offline durable retry | — | ⚠️ in-memory only | ⚠️ | ? | persistent retry | ⚠️ relay no durable queue (cloud has 1024 cap) | P2 |
| Cloud (Supabase) sync/retry/realtime | cloud.rs | ✅ retry 1024, WS+poll | ✅ | ✅ workers | full sync | ✅ | — |
| Cloud sensitive exclusion | cloud.rs:1007-1016 | ✅ | ✅ | ✅ | never upload | ✅ | yes |
| TTL / delete propagation | sync_orch.rs | ✅ tombstones+LWW | ✅ | ✅ | propagate | ✅ | — |
| Platform transport support | — | all | P2P+relay+cloud | relay+cloud | document deltas | ⚠️ Android P2P gap undocumented | P2 |

## 11. Android-specific limitations (document each; undocumented = bug)

| Limitation | Closest safe equivalent | Documented? | sev |
|---|---|---|---|
| No daemon / no Unix-socket IPC (UniFFI node) | embeds core directly | partially (CLAUDE.md) | — |
| No P2P/mTLS LAN sync | relay / cloud transport | ❌ (add to known-issues) | P2 |
| No history export/import/backup | macOS CLI | ❌ | P1 |
| No undo-on-delete | confirm dialog | ❌ | P1 |
| No degraded-DB reset affordance | OS "clear app data" | ❌ | P2 |
| Single active SyncBackend | choose relay OR cloud | ⚠️ partial | P2 |
| API<31 blur fallback (bullet mask) | bullet mask | ✅ (code comment) | — |

## 12. macOS-specific limitations

| Limitation | Closest safe equivalent | Documented? | sev |
|---|---|---|---|
| No GUI vacuum/stats/backup | CLI | ⚠️ partial | P2 |
| No full daemon-lifecycle GUI | RestartDaemonButton + launchctl | ⚠️ partial | P3 |
| Windows frozen | macOS/Android/Linux-daemon | ✅ ADR-012 | — |

---

### Fix owners (suggested)
- **Android app team:** P1-2 (undo), P1-1 (export/import FFI+UI), P2-1/P2-2 (transport), P2-5 (recovery), P2-6/P2-7 (devices), P3-1/P3-2/P3-5 (UI).
- **macOS-UI team:** P2-4 (vacuum/stats UI), P2-3 (backup UI).
- **Docs/design:** P1-3 (theme-default spec reconciliation), P3-3/P3-4 (comments).
- **Refuted (no owner):** clear-all, web Verified badge, sensitive-FTS, relay-sensitive, token drifts — all verified correct.
