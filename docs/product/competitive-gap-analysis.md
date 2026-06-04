# CopyPaste — Competitive Gap Analysis

**Branch:** v0.6.1-integration  
**Date:** 2026-06-04  
**Scope:** Clipboard manager landscape — macOS, Windows, Android, iOS, cross-platform/cloud-sync

---

## Executive Summary (7 biggest gaps)

1. **No snippets/templates system.** Every major competitor (Alfred, Raycast, Paste, Pastebot, PastePal, ClipboardFusion) ships a first-class snippet/template engine with text auto-expansion. CopyPaste has clipboard history but no way to store reusable text with placeholders.
2. **No iOS/iPadOS app.** Paste, PastePal, CleanClip, and ClipCascade all reach iPhone/iPad. CopyPaste is macOS + Android only; iPhone users have no client, making it a non-starter for Apple-centric households.
3. **No paste-as-plain-text / multi-format paste.** Raycast, ClipBook, Alfred, and Pastebot let users choose the paste format (plain text / RTF / HTML) at paste time. CopyPaste pastes in the format it captured — no stripping, no format switching.
4. **No OCR / image text search.** Raycast, Paste (Apple Intelligence, Nov 2025), and CleanClip can search the text inside copied images. CopyPaste stores images but cannot search them by content.
5. **No per-app exclusion rules.** Paste, Maccy (via integration), PastePal, Ditto, and ClipboardFusion support blocking specific apps (e.g. 1Password) from being captured. CopyPaste's only escape hatch is private mode (all-or-nothing), or relying on sensitive-item TTL.
6. **No browser extension.** Clipboard History Pro, ClipboardFusion, and multiple cross-platform managers offer Chrome/Edge/Firefox extensions. CopyPaste has no web-browser integration path.
7. **No AI / MCP integration.** Paste launched a local MCP server in June 2026 to feed clipboard history into Claude, Codex, and Cursor. Raycast integrates directly into AI workflows. CopyPaste has no AI-augmentation story yet.

---

## 1. Capability Matrix

Competitors selected: **Raycast** (macOS, freemium), **Paste** (macOS + iOS, subscription), **Alfred** (macOS, Powerpack), **Maccy** (macOS, free/open-source), **Ditto** (Windows, free/open-source), **ClipboardFusion** (Windows, free/pro), **Pastebot** (macOS, $12.99 one-time), **PastePal** (macOS + iOS, one-time), **ClipCascade** (cross-platform self-hosted).

| Feature | CopyPaste | Raycast | Paste | Alfred | Maccy | Ditto | ClipboardFusion | Pastebot | PastePal | ClipCascade |
|---|---|---|---|---|---|---|---|---|---|---|
| **Clipboard history** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Persistent history across reboots** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Full-text search** | ✓ (FTS5) | ✓ | ✓ | ✓ | ✓ | ✓ | partial | ✓ | ✓ | partial |
| **Image capture & preview** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| **File capture** | ✓ | ✓ | ✓ | partial | ✗ | ✓ | partial | ✓ | ✓ | ✓ |
| **Pin / favorite items** | ✓ | ✓ | ✓ (Pinboards) | ✓ | ✓ | ✓ (groups) | ✓ | ✓ | ✓ | partial |
| **Pinned item reorder** | ✓ | ✗ | ✓ | ✗ | ✗ | ✓ | ✓ | ✓ | ✓ | ✗ |
| **Sensitive item detection** | ✓ (regex) | ✓ (pw mgr) | ✓ (pw mgr) | ✓ (pw mgr) | ✓ (pw mgr) | partial | partial | ✓ | ✓ | partial |
| **Sensitive item TTL / auto-purge** | ✓ | partial | partial | ✗ | partial | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Per-app exclusion rules** | ✗ | ✓ | ✓ | ✓ | partial | ✓ | ✓ | ✓ | ✓ | ✗ |
| **Private mode (capture off toggle)** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Source app badge** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| **Content-kind classification** | ✓ (URL/email/code/color/…) | ✓ | ✓ | partial | ✗ | partial | partial | partial | ✓ | ✗ |
| **Snippets / templates** | ✗ | ✓ | partial (Pinboards) | ✓ | ✗ | partial | ✓ | ✓ (filters+boards) | partial | ✗ |
| **Text auto-expansion (type keyword → expand)** | ✗ | ✓ | ✗ | ✓ | ✗ | ✗ | ✓ (triggers) | ✗ | ✗ | ✗ |
| **Dynamic placeholders (date, cursor, fill-in)** | ✗ | ✓ | ✗ | ✓ | ✗ | ✗ | ✓ (C# macros) | partial | ✗ | ✗ |
| **Paste-as-plain-text** | ✗ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| **Multi-format paste (choose RTF/HTML/plain at paste time)** | ✗ | ✓ | partial | ✓ | ✗ | ✓ | ✓ | partial | ✓ | ✗ |
| **RTF / rich text preservation** | ✗ | ✓ | ✓ | ✓ | ✗ | ✓ | ✓ | ✓ | ✓ | ✗ |
| **HTML clipboard format preservation** | ✗ | ✓ | ✓ | ✓ | ✗ | ✓ | ✓ | partial | ✓ | ✗ |
| **OCR / image text search** | ✗ | ✓ | ✓ (Apple Intelligence, Nov 2025) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Merge/append clips (Cmd+C+C)** | ✗ | ✗ | ✗ | ✓ | ✗ | ✓ | ✓ (macros) | ✗ | ✗ | ✗ |
| **Paste stack / paste queue (sequential multi-paste)** | ✗ | ✗ | partial | partial (workflow) | ✗ | ✗ | ✗ | ✗ | ✓ (Queue) | ✗ |
| **Numeric/hotkey shortcuts to paste specific items** | ✗ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (up to 30) | ✓ | ✓ | ✗ |
| **Folders / collections / pinboards for organization** | ✗ | ✗ | ✓ (Pinboards) | ✓ (snippet collections) | ✗ | ✓ (groups) | partial | ✓ (custom boards) | ✓ (collections) | ✗ |
| **Tags on history items** | ✗ | ✗ | ✗ | ✗ | ✗ | partial | ✗ | ✗ | partial | ✗ |
| **Text transformation at paste time** | ✗ | ✓ (case/etc) | ✗ | ✓ | ✗ | ✗ | ✓ (75+ PastePal-style + C# macros) | ✓ (filters) | ✓ (75+ transforms) | ✗ |
| **Edit clip before pasting** | ✗ | ✓ | ✓ | partial | ✗ | ✓ | ✓ | partial | partial | ✗ |
| **End-to-end encryption (E2E)** | ✓ (XChaCha20-Poly1305) | ✗ (local only) | ✗ (iCloud TLS) | ✗ | ✗ | partial (AES transit) | partial (256-bit in-transit) | ✗ (iCloud TLS) | ✗ (iCloud TLS) | ✓ (E2E) |
| **Self-hostable relay / server** | ✓ | ✗ | ✗ | ✗ | ✗ | partial (LAN share) | partial | ✗ | ✗ | ✓ (Docker) |
| **Cross-device sync** | ✓ (3 paths: P2P/relay/Supabase) | ✗ (local Mac only) | ✓ (iCloud) | ✗ | ✗ | ✓ (Windows LAN) | ✓ (Pro, cloud) | ✓ (iCloud, Mac only) | ✓ (iCloud, peer LAN) | ✓ |
| **macOS client** | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ | ✓ | ✓ | ✓ |
| **Android client** | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ (Pro sync) | ✗ | ✗ | ✓ |
| **iOS / iPadOS client** | ✗ | ✓ | ✓ | ✗ | ✗ | ✗ | ✓ (Pro sync) | partial (Universal Clipboard bridge) | ✓ | ✗ |
| **Windows client** | frozen | ✗ | ✗ | ✗ | ✗ | ✓ | ✓ | ✗ | ✗ | ✓ |
| **Linux client** | ✓ (daemon) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| **Browser extension (Chrome/Edge/Firefox)** | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | partial | ✗ | ✗ | ✗ |
| **Spotlight integration** | ✗ | replaces Spotlight | ✗ | replaces Spotlight | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **AI / MCP integration** | ✗ | ✓ (built-in AI commands) | ✓ (MCP server, June 2026) | ✓ (workflows) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Configurable popup hotkey** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| **Sound / notification on copy** | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Configurable storage quota** | ✓ | ✗ | ✗ | ✓ (by time) | ✓ (by count) | ✓ (by count) | ✓ | ✓ (by count) | ✓ | ✓ |
| **Export clipboard history** | partial (CLI) | ✗ | ✗ | ✗ | ✗ | ✓ | ✓ | ✗ | ✗ | ✗ |
| **Encrypted local storage** | ✓ (SQLCipher) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| **Device revocation / key rotation** | ✓ | n/a | ✗ | n/a | n/a | ✗ | partial | ✗ | ✗ | ✗ |
| **QR pairing / zero-config device setup** | ✓ | n/a | n/a | n/a | n/a | ✗ | ✗ | n/a | ✗ | ✗ |
| **SAS verification (PAKE security)** | ✓ | n/a | n/a | n/a | n/a | ✗ | ✗ | n/a | ✗ | ✗ |
| **Shared / team clipboard (multi-user)** | ✗ | ✗ | ✓ (Shared Pinboards, 2025) | ✗ | ✗ | ✓ (LAN share) | ✓ | ✗ | ✗ | partial |

---

## 2. Where We Lag Behind Competitors

Each item includes: what it is, who does it best, why it matters, rough effort (S = days, M = 1–3 weeks, L = month+).

### GAP-1 — Snippets / Templates with Text Auto-Expansion
**What:** A separate store of reusable text fragments ("snippets") that can be triggered by a short keyword typed anywhere — the keyword auto-expands to the full template. Supports dynamic placeholders like `{date}`, `{cursor}`, and fill-in prompts.  
**Who does it well:** Alfred (Powerpack) is the gold standard; Raycast Snippets is a close second; ClipboardFusion supports C# macro triggers; Pastebot's Filters handle transformation on paste.  
**Why it matters:** Snippets are the #1 workflow accelerator for knowledge workers — email signatures, boilerplate, code templates. Users who outgrow clipboard history immediately look for snippet support; its absence pushes them to TextExpander, Raycast, or Alfred where they gain an integrated clipboard too. Without snippets, CopyPaste cannot compete for the power-user segment.  
**Effort:** L (needs a separate storage model, keyword index, auto-expansion hook at the OS event-tap level, and UI for managing snippet collections).

### GAP-2 — iOS / iPadOS Client
**What:** A native iPhone/iPad app with the same history, search, and paste capabilities as macOS. At minimum: a custom keyboard extension for in-app pasting, and a share extension to ingest content.  
**Who does it well:** Paste (best-in-class — iPhone/iPad native app, custom keyboard, pinboards), PastePal (universal purchase Mac + iOS), CleanClip (iOS beta as of Jan 2026).  
**Why it matters:** iPhone is the dominant mobile device in the Apple ecosystem. Any user who wants a seamless Mac↔iPhone clipboard workflow is blocked today — they cannot even read their CopyPaste history on iPhone, let alone paste from it. Paste charges $30/year precisely because this cross-device Apple story is compelling. CopyPaste already does Mac↔Android well; the absence of iOS makes it unattractive to the majority of Mac users who also own an iPhone.  
**Effort:** L (requires a new SwiftUI app with custom keyboard extension and share extension; iCloud or relay/Supabase sync path needs iOS SDK wrappers; the Rust core would need a Swift/iOS UniFFI build).

### GAP-3 — Paste-as-Plain-Text / Multi-Format Paste
**What:** When pasting, let the user choose what format to paste in — plain text (stripping HTML/RTF/formatting), rich text, or the full original multi-format bundle. Maccy, Raycast, Alfred, Ditto, Pastebot, PastePal, and ClipboardFusion all offer at least a "paste without formatting" action.  
**Who does it well:** Raycast ("Paste as…" with RTF / HTML / plain text / any source format); ClipBook (multi-format stored and switch-able in preview panel); Maccy (paste without formatting is a core option).  
**Why it matters:** Pasting from a browser into Slack, Notion, or a terminal almost always results in unwanted HTML formatting. "Paste without formatting" (⌘⇧V equivalent) is the single most-requested power feature in every clipboard manager forum. The absence of this in CopyPaste is a daily friction point for users who copy from web pages.  
**Effort:** M (macOS: change the NSPasteboard write path to offer the item in `NSString` only when a "plain text" paste action is used; Android: similar plaintext-only write to `ClipboardManager`; UI: add a secondary action in the popup/history row).

### GAP-4 — OCR / Search Inside Images
**What:** When an image is captured, run on-device OCR and index the extracted text, so searching "invoice" finds a screenshot of an invoice, and searching "192.168.1.1" finds a screenshot containing that IP.  
**Who does it well:** Raycast (built-in OCR via Vision framework, searches clipboard images and screenshots); Paste (Apple Intelligence integration, November 2025, adds OCR image search).  
**Why it matters:** A large share of clipboard items are screenshots — error messages, UI designs, photos of text. Today those are dead weight in search. OCR indexing turns them into searchable history. This is increasingly a table-stakes feature as Raycast and Paste have both shipped it.  
**Effort:** M (macOS: Apple Vision framework `VNRecognizeTextRequest` at capture time; store extracted text in `clipboard_fts` alongside the item; no UI changes beyond making search results include images).

### GAP-5 — Per-App Exclusion Rules
**What:** A configurable list of apps (by bundle ID on macOS, package name on Android) whose clipboard contents CopyPaste will never capture or store. The canonical use case: exclude 1Password, Bitwarden, banking apps.  
**Who does it well:** Paste (configurable allow/ignore lists); PastePal (allow and ignore lists with app picker); Maccy (integrated respects transient items; separate ignore list toggle); Ditto (ignore list); ClipboardFusion (triggers by source app).  
**Why it matters:** Today CopyPaste captures passwords unless private mode is on or the item's regex matches a sensitive pattern. Users from banking apps, corporate SSO tools, or custom internal tools won't match the regex. A per-app blocklist is the correct defense-in-depth layer and is required by enterprise security teams. It is also the "obvious missing feature" for anyone who tries CopyPaste after migrating from Paste or Maccy.  
**Effort:** S–M (macOS: read `NSPasteboard` owner bundle ID at poll time — already captured as `app_bundle_id`; add an exclusion list in `AppConfig`; skip storage when bundle ID matches; UI: app-picker in Settings).

### GAP-6 — Folders / Collections for Clipboard Organization
**What:** Named collections (Paste calls them Pinboards, Alfred uses Snippet Collections, PastePal uses Collections) where users manually drag or rule-assign items. This is beyond simple pinning — it is a filing system for reusable content.  
**Who does it well:** Paste (Pinboards — can share with team, supports iCloud sync); PastePal (Collections, searchable by type); Pastebot (Custom Pasteboards with keyboard shortcuts to invoke); Ditto (Groups).  
**Why it matters:** Long-lived clips — code snippets, meeting agendas, design color palettes — currently have no home in CopyPaste other than being pinned in a flat list. As pinned items grow, the list becomes unnavigable. Collections/Pinboards allow users to have, say, "Work boilerplate", "Design tokens", and "Personal info" boards that survive history purges and can be curated independently.  
**Effort:** M (requires new DB table for collections, many-to-many item↔collection relation, UI for collection creation/management, and separate IPC methods; syncing collections across devices is L if needed).

### GAP-7 — Text Transformation at Paste Time
**What:** Apply a transformation to clipboard content just before pasting — e.g. Title Case, lowercase, trim whitespace, Base64 encode/decode, URL encode, Strip HTML, JSON prettify. Invoked from the history popup via a secondary action.  
**Who does it well:** PastePal (75+ built-in transforms, live preview); Pastebot (Filters with live preview, shareable filter files); ClipboardFusion (C# macro scripts, arbitrary transformations); Alfred (via workflows).  
**Why it matters:** Developers and writers constantly need to massage clipboard content — uppercasing, stripping tags, encoding. Today users switch to a terminal or a separate tool. Inline transformations keep the flow in CopyPaste and are a strong differentiator against lightweight managers like Maccy.  
**Effort:** M (core: a set of pure text-transform functions; daemon/IPC: `transform_item` method returning transformed text without storing; UI: action sheet in history popup).

### GAP-8 — Merge / Append Clips
**What:** Rapidly select multiple clipboard entries and merge them into one (concatenation, optionally with a separator). Alfred's Cmd+C+C shortcut appends the newly copied text to the previous clipboard item; Ditto has a multi-select + merge; Clip Stack on Android has a merge action.  
**Who does it well:** Alfred (Cmd+C+C append to previous); Ditto (explicit merge action); Clip Stack (Android, merge action); ClipboardFusion (macro).  
**Why it matters:** Gathering fragments from multiple sources into one cohesive block (e.g. collecting code from several files) requires repeated manual edit sessions today. Merge/append lets users do this in seconds without switching to an editor.  
**Effort:** S (UI: multi-select + merge action in history view; daemon: `merge_items` IPC method that concatenates and stores a new item).

### GAP-9 — AI / MCP Integration
**What:** A local MCP (Model Context Protocol) server that exposes the clipboard history as context to AI assistants (Claude, Codex, Cursor, etc.), plus possibly an inline AI action (summarize, translate, rewrite) on selected clipboard content.  
**Who does it well:** Paste (local MCP server launched June 2026, direct integration with Claude/Codex/Cursor); Raycast (AI Commands built in, clipboard content accessible in prompts).  
**Why it matters:** AI assistant usage has become ubiquitous in development and writing workflows. Paste's MCP launch received immediate coverage (9to5Mac, iGeeksBlog). Clipboard context is exactly what AI tools need — recent code, error messages, URLs, copied data. Without MCP, CopyPaste users must manually copy-paste from the history into their AI tool, breaking flow. This is a fast-moving table-stakes feature.  
**Effort:** M (build a local HTTP/stdio MCP server that reads from the daemon via the existing Unix socket; expose tools like `list_recent_clips`, `search_clips`, `get_clip_by_id`; no new Rust required — can be a thin TypeScript/Node shim if speed is a concern).

### GAP-10 — Paste Stack / Sequential Multi-Paste
**What:** A queue of items to paste one-by-one in sequence — user loads the queue, then each paste keystroke consumes the next item. Useful for filling in multiple form fields, inserting code sections in order, or multi-paragraph responses.  
**Who does it well:** PastePal (Queue feature, paste all at once or one-by-one); CleanClip (Paste Stack mode); Alfred PasteFlow workflow; Paste (paste multiple items in chosen order).  
**Why it matters:** Data entry workflows — filling a form with 5 fields, inserting structured code in order — currently require the user to navigate the history between each paste. A paste queue removes this friction entirely. It is a niche feature but has zero competition from operating-system built-ins.  
**Effort:** S–M (daemon-side: a transient ordered list IPC endpoint; UI: queue management overlay in popup/history view; no new storage model needed).

### GAP-11 — Browser Extension
**What:** A Chrome/Edge/Firefox extension that captures text selected on web pages, provides a floating paste button, or offers quick access to clipboard history in a browser context.  
**Who does it well:** Clipboard History Pro (Chrome extension with cloud sync to Android/iOS); ClipboardFusion (partial via sync); various standalone Chrome extensions.  
**Why it matters:** Web-based workflows (form filling, research, writing in Google Docs/Notion) represent a large portion of clipboard use. A browser extension lets CopyPaste stay useful even when the Tauri popup is inaccessible (browser full-screen, kiosk mode) and opens a path to cross-OS clipboard access without a native app.  
**Effort:** L (requires a standalone extension codebase, a sync/communication mechanism to the daemon, and handling the cross-origin clipboard security model; not trivial).

### GAP-12 — Numeric / Per-Item Hotkeys for Direct Paste
**What:** Assign a global hotkey to paste a specific history position or a pinned item directly, without opening the popup. "Ctrl+1" always pastes the most recent item, "Ctrl+2" the second, or users assign hotkeys to specific pinned items.  
**Who does it well:** Ditto (up to 10 hotkeys for history positions); ClipboardFusion (up to 30 configurable hotkeys); Pastebot (per-pasteboard keyboard shortcuts); Maccy (configurable shortcut per position).  
**Why it matters:** Power users who repeatedly paste a fixed set of items (email signature, API test payload, standard greeting) want zero-UI access — one keystroke with no popup. Today every CopyPaste paste requires opening the popup, navigating, selecting.  
**Effort:** S (Tauri global shortcut registration for user-defined pinned-item shortcuts; daemon: `paste_pinned_at_index` IPC method).

---

## 3. Where We Are Ahead

### Genuine Differentiators (be honest — don't overclaim)

**3.1 True End-to-End Encryption with Cryptographically Sound Design**  
CopyPaste is the only mainstream clipboard manager (alongside ClipCascade) that encrypts content E2E — meaning the relay server and cloud storage never hold decryption keys. XChaCha20-Poly1305 with 192-bit random nonces, HKDF-SHA256 key derivation, AEAD AAD binding `(item_id, schema_version, key_version)`, and constant-time comparisons via the `subtle` crate. Competitors that use iCloud (Paste, Pastebot, PastePal) rely on Apple TLS/iCloud encryption — respectable, but Apple holds the keys. Raycast encrypts locally only and has no sync at all.

**3.2 Three Independent Sync Paths**  
P2P (LAN mTLS with mDNS-SD), relay-as-database (self-hostable), and Supabase cloud — all three can run simultaneously and independently, with the same E2E ciphertext on all paths. No competitor offers this resilience architecture. ClipCascade supports P2P + server but lacks the three-path redundancy.

**3.3 Authenticated Device Pairing (PAKE + SAS)**  
Devices pair over a PAKE bootstrap channel and confirm with a 6-digit Short Authentication String. This prevents MITM attacks on pairing. No other clipboard manager in the landscape implements a comparable cryptographic pairing ceremony — competitors use iCloud account matching, LAN password, or QR codes that embed the key material directly.

**3.4 Device Revocation and Key Rotation**  
The `revoke_and_rotate` IPC method lets users cut off a lost device by rotating the sync key in one atomic operation. All remaining devices re-derive keys from the new passphrase; the revoked device's relay inbox ID diverges immediately. No iCloud-based competitor offers this (iCloud has no per-device key isolation).

**3.5 True macOS↔Android Cross-Platform Sync**  
The only other cross-platform E2E manager reaching both macOS and Android is ClipCascade (also open source, self-hosted). Commercial competitors do not touch Android: Paste, Pastebot, PastePal, Maccy, Raycast are all Apple-only. ClipboardFusion reaches Android but via a proprietary cloud, not E2E.

**3.6 Sensitive Item TTL with Automatic Purge**  
Time-bounded auto-purge for sensitive items (passwords, API keys) is unique to CopyPaste in the Mac manager space. The `expires_at` field is set at capture time and purged on each daemon tick — no other tested competitor ships automatic expiry for sensitive content (they rely on the password manager clearing the system clipboard, which only works for the initial copy, not subsequent copies from the clipboard manager itself).

**3.7 Rich Content-Kind Classification**  
The daemon classifies text items into: TEXT, URL, EMAIL, PHONE, COLOR, JSON, CODE, NUMBER, PATH — surfaced in the UI as badges. PastePal does something similar; most competitors offer only "text / image / file" tripartite.

**3.8 Encrypted Local SQLCipher Database**  
Local history is encrypted at rest using SQLCipher. Competitors store plaintext SQLite or NSUserDefaults. If a machine is stolen, CopyPaste history is unreadable without the device key (held in macOS Keychain).

---

## 4. Top 10 Highest-Impact Gaps to Close (Ranked)

Priority is based on estimated number of affected users, frequency of friction, and competitive table-stakes status.

| Rank | Gap | Competitors ahead | User impact | Effort |
|---|---|---|---|---|
| 1 | **Paste-as-plain-text / multi-format paste** | Raycast, Maccy, Alfred, Pastebot, PastePal, Ditto | Every user, every day — pasting from browser creates HTML formatting mess | M |
| 2 | **Per-app exclusion rules** | Paste, PastePal, Maccy, Ditto, ClipboardFusion | Security/privacy blocker for enterprise, 1Password users, banking apps | S–M |
| 3 | **iOS / iPadOS client** | Paste, PastePal, CleanClip | Blocks all Apple-ecosystem households; largest unsupported platform | L |
| 4 | **AI / MCP integration** | Paste (MCP, June 2026), Raycast | Fast-moving table-stakes; Paste just shipped; developer users will notice | M |
| 5 | **Snippets / templates** | Alfred, Raycast, Pastebot, PastePal, ClipboardFusion | Power-user retention; primary reason people adopt Alfred/Raycast over a pure clip manager | L |
| 6 | **OCR / image text search** | Raycast, Paste (Apple Intelligence) | Screenshot-heavy workflows; increasingly expected; Apple Intelligence made it easy | M |
| 7 | **Numeric / per-item hotkeys for pinned items** | Ditto, ClipboardFusion, Pastebot, Maccy | Low-friction direct paste without popup; common power-user request | S |
| 8 | **Text transformation at paste time** | PastePal (75+), Pastebot (Filters), ClipboardFusion | Developer and writer audience; differentiates from simple clip managers | M |
| 9 | **Collections / Pinboards** | Paste, PastePal, Pastebot, Ditto | Long-term curation; pins alone do not scale beyond ~10–15 items | M |
| 10 | **Merge / append clips** | Alfred (Cmd+C+C), Ditto, Clip Stack | Compound-copy workflow; removes a common multi-step workaround | S |

**Honourable mentions (just outside top 10):** paste stack/queue (CleanClip, PastePal), team shared clipboard (Paste Shared Pinboards), browser extension (Clipboard History Pro), configurable history-size by count vs. item count vs. time window.

---

## 5. Methodology Notes

- CopyPaste feature inventory derived from: `README.md`, `ARCHITECTURE.md`, `crates/copypaste-ui/src/lib/ipc.ts` (full IPC surface), `crates/copypaste-ui/src/views/SettingsView.tsx` (settings tabs: General, Display, Sync, Shortcuts, Storage, Advanced), and `crates/copypaste-ui/src/views/HistoryView.tsx`.
- Competitor features verified via: official product websites, App Store listings, GitHub repositories, and independent reviews (Zapier, MacStories, 9to5Mac, XDA Developers, MakeUseOf) as of June 2026.
- Where a feature's presence is uncertain for a competitor, the cell shows "partial" and the claim is conservative. No competitor feature was fabricated.
- Effort estimates are for a single experienced Rust/TypeScript developer; platform-parity work (macOS + Android) roughly doubles them.
