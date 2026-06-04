# Persona Critique: Power User / Software Developer

**Role:** Senior backend engineer. Multiple machines: MacBook Pro (primary), Android phone (Pixel 8), Linux workstation at home. Copies and pastes constantly — curl commands, JSON payloads, git SHAs, API keys, SQL snippets, stack traces, base64 blobs, Markdown table skeletons, error messages. Has used Maccy for two years, Raycast for one, briefly tried Paste.

---

## 1. What Works Well for Me

**The security story is the first thing I actually trust.** XChaCha20-Poly1305 at rest, AEAD AAD bound to `(item_id, schema_version, key_version)`, SQLCipher, macOS Keychain-backed. No other clipboard manager I have evaluated comes close on this axis. When I copy an AWS key or a JWT, I am not happy about it sitting in a plaintext NSUserDefaults file (looking at every competitor here). The auto-wipe TTL for sensitive items is genuinely useful — 30 seconds default means a copied credential that I forgot about does not linger in my history forever.

**The content-kind classification is immediately practical.** The `KindChip` badges — URL, CODE, JSON, EMAIL, PATH — let me visually scan a long history at a glance instead of reading every line. When I am hunting for the API endpoint I copied twenty minutes ago, I skip everything that is not labelled URL. This is better than any other tool I have used.

**The global hotkey popup is fast and does what I need most of the time.** `Cmd+Shift+V` → fuzzy search → `Enter`. That is a three-keystroke paste from history. The `⌘1`–`⌘9` direct-paste shortcuts for the first nine items are useful during repetitive data-entry tasks. The popup auto-focuses the search bar and the fuzzy match is good enough for most queries.

**Cross-device sync that does not require me to hand my data to a third party.** Three independent paths — P2P LAN, self-hosted relay, Supabase — all E2E encrypted with client-side re-keying. The PAKE pairing with SAS verification is the right crypto design. I paired my phone once, never thought about it again. When I am on the same LAN, P2P kicks in and clips appear in under two seconds.

**Pinning and reorder work correctly on macOS.** I have about twelve pinned items that are permanent fixtures: a curl template, a local DB connection string stub, a base64 decoding command, my most-used SQL scaffold. Drag-to-reorder in the history view and the pins-survive-copy-bump behavior are both correct. These seem obvious but other tools botch them.

**Sensitive detection is broader than password-manager heuristics.** The 37-pattern regex set catches AWS keys, GitHub PATs, OpenAI tokens, Stripe keys, Heroku keys, HashiCorp Vault tokens, SSH private keys, database connection strings, `.env` assignments. Every other clipboard manager I have used catches exactly zero of these — they rely on the source app annotating with `org.nspasteboard.ConcealedType`, which most apps do not do. The regex approach with 0.99-confidence patterns is more reliable.

**The CLI exists.** `copypaste-cli` speaking IPC to the daemon means I can script clipboard interactions. That is a huge deal for a developer. I can `copypaste list`, I can pipe into it. The daemon's Unix socket is a proper API.

---

## 2. What Is Inconvenient or Frustrating

**Paste-as-plain-text is buried and all-or-nothing.** There is a global `paste_as_plain_text` toggle in Settings → General that strips RTF/HTML on every paste from history. I cannot do it per-item at paste time. My workflow is: copy a snippet from a web page, open the popup, and paste it into my terminal or a Markdown file. I want plain text in those contexts. But when I paste into Notion or Confluence I want the rich format. Forcing a global toggle means I constantly toggle it and forget to toggle back, or I paste garbage HTML into my terminal. Every other tool I use — Raycast, Alfred, Maccy — solves this with a secondary paste action (e.g. `Option+Enter` = paste as plain text) in the popup. The absence of this is a daily friction point. I have to use `pbpaste | pbcopy` in a terminal more often than I should need to with a clipboard manager installed.

**Search in the main history window is client-side substring over at most 1,000 items, and is disabled during load-more.** The FTS5 infrastructure is there in the daemon — `clipboard_fts` is populated at insert time — but the macOS UI calls a client-side substring match over the loaded page instead of invoking FTS through IPC. If I have been running for a week and accumulated 3,000 items, my query for "pg_dump" finds nothing because the item was on page 3. I only discovered this after wondering why search was missing things I knew were there. This is a real bug disguised as a limitation — the engine exists, it is just not wired to the UI.

**The popup has no shortcut to open full history.** I hit `Cmd+Shift+V`, search, find nothing in the 50-item popup, and there is no `Cmd+Enter` or keyboard shortcut to jump to the full history view with the same query pre-filled. The gear icon in the footer goes to Settings, not History. I have to close the popup, manually open the main window, and re-type the search. Two separate surfaces with no handoff.

**No per-item "copy as plain text."** Even if I do not want the global toggle, I should be able to right-click or use a keyboard shortcut on a specific item to paste it without formatting. The UI has hover actions (Eye, Pin, Delete) but no format-switch action.

**Android search is O(n) full decryption.** The `searchIds` function decrypts every item's full text per search call. With hundreds of items this is perceptible lag on a phone. The macOS daemon does FTS5 properly; Android uses SharedPreferences with no index. This is a backend architecture difference that surfaces as a real slowness in daily use.

**Pinned reorder is not available in the popup.** This is minor but: I have pinned items in a specific order (most-used first). In the popup they appear correctly ordered, but if I want to change the order I have to open the main window. Not a dealbreaker but an inconsistency.

**The QR pairing only provisions P2P, not cloud credentials.** I scanned the QR from my Mac on my phone, pairing worked fine for P2P on the same LAN. Then I moved to a cellular connection and sync stopped. I had to go manually type the Supabase URL, anon key, relay URL, and passphrase into Settings on my phone. The QR payload has all the room in the world to carry the provisioning material; this was a surprising omission for a tool whose QR-pairing is otherwise well-designed.

**Delete has no undo.** Clicking the trash icon on a row deletes it silently and immediately. As a power user I frequently delete items while navigating quickly and I have fat-fingered it. A 3-second "Deleted — Undo" toast would fix this entirely and the soft-delete tombstone infrastructure for LWW op-propagation already exists in the schema.

**The "Advanced" settings tab is an empty placeholder.** I opened it expecting daemon tuning options — poll interval, FTS settings, relay configuration, maybe raw config export. I got "Advanced daemon and storage limits will appear here in a future release." If you have a tab with that name, put something in it. Every developer instinctively opens the Advanced tab first.

**Remote device badge shows `3a7f1b2c` (a UUID prefix), not my device name.** Items synced from my phone show an 8-character hex fragment. I know from context which device it is, but I should not have to know — the paired-peers roster has the name, it is just not being resolved at render time.

---

## 3. What Is Missing That I Would Expect

These are ranked by how often the absence blocks my workflow.

### 3.1 Snippets / Templates — CRITICAL DAILY GAP

I have about 30 text fragments I reuse constantly: a `curl -X POST` scaffold with Authorization header, a Docker container cleanup one-liner, a pg_dump connection string template, a Jira ticket format template, a standard PR description skeleton, a `git log --oneline --graph --decorate` alias I always forget the flags for. Right now I keep these in a separate notes app and context-switch to them.

CopyPaste has pinned items, which is a partial substitute, but it is not a snippet system. A snippet system needs: a permanent store that does not participate in history TTL or quota eviction, keyword triggering (type `;curl` → expands to the full command), placeholder fields (`{hostname}`, `{date}`), and a collection/folder structure so 30 snippets do not cram into one flat pinned list. Alfred and Raycast both do this. The absence of snippets is the single biggest reason I have not fully switched from Raycast, which I still keep running purely for its snippets feature.

Frequency: I use snippets 15–20 times per day.

### 3.2 Paste-as-Plain-Text Per-Item Action — HIGH DAILY FRICTION

Covered in §2. But to be explicit about what I want: an `Option+Enter` (or configurable) secondary paste action in the popup that writes only `NSPasteboardTypeString`, ignoring RTF/HTML/any other format the item was captured with. Not a global toggle. Per-paste. Every major competitor ships this. This should take a week to implement.

Frequency: multiple times per day.

### 3.3 Text Transformations at Paste Time — REGULAR USE

Before pasting, I often need: strip HTML tags, URL-encode, base64-encode, JSON prettify, lowercase, trim leading/trailing whitespace. Right now I pipe through `python3 -c`, `jq`, `base64`, etc. in a terminal. PastePal ships 75 of these. Pastebot calls them Filters. Even a small set — JSON prettify, base64 decode, URL encode/decode, strip HTML, UPPER/lower/Title case — would remove constant context switches to the terminal for text massaging.

Frequency: 5–10 times per day.

### 3.4 FTS Search Wired to UI — SHOULD ALREADY WORK

The FTS5 table is populated. The daemon has the search infrastructure. The UI does client-side substring on a 1,000-item page. This is not a missing feature, it is a missing wire-up. I would file this as a bug. I expect to search my entire history, not just what happens to be loaded.

Frequency: every time I search anything beyond the last hour of history.

### 3.5 Direct Hotkeys for Pinned Items — FREQUENT

I want `Cmd+Option+1` to paste my first pinned item without opening any UI. `Cmd+Option+2` for the second, and so on for the top 5 or so. Ditto supports up to 10 such hotkeys, ClipboardFusion up to 30. The ergonomics of "open popup → navigate to pinned section → press Enter" is 3 steps when it could be 1. For truly fixed, reused content (DB connection string, SSH key comment template), this removes the popup entirely from the hot path.

Frequency: 10–15 times per day for my top 3 pinned items.

### 3.6 Merge / Append Clips — MODERATE USE

Alfred's `Cmd+C+C` behavior: if you press Cmd+C twice in rapid succession it appends the new copy to the previous clipboard, separated by a newline. This is incredibly useful for collecting fragments from different files or windows without switching to an editor to accumulate them. I miss this from Alfred constantly. The daemon already supports bulk-copy with newline concatenation in multi-select — extending this to a global double-tap intercept would be the right design.

Frequency: 3–5 times per day when building composite snippets or collecting error lines from logs.

### 3.7 MCP Server for AI Tools — GROWING RAPIDLY

I run Claude and Cursor daily. Both support MCP. Right now when I want to give the AI context about something I copied earlier (an error message, an API response, a config fragment), I have to manually navigate the popup, find the item, and paste it into the AI chat. Paste just launched an MCP server for exactly this. The CopyPaste daemon already exposes a Unix socket IPC with `history_page` and `search` methods — a thin MCP wrapper around that is probably a weekend project. The use case: "here are the last 10 items I copied, use them as context" or "search my clipboard history for anything that looks like a database connection string."

Frequency: becoming daily; will be critical in 6 months.

### 3.8 Per-App Exclusion Rules — SECURITY BASELINE

The `excluded_app_bundle_ids` field exists in `AppConfig` and the UI has a bundle-ID input in Settings → General. But on Android the equivalent is missing. And there is no UI-level app picker — I have to know the bundle ID for every app I want to exclude. More importantly, there are apps that will never match any sensitive-content regex but that I absolutely do not want captured: my company's internal SSO tool, a custom password vault, a trading app. The per-app exclusion is not a power-user nice-to-have — it is a security baseline that enterprise users and security-conscious developers expect on day one.

Frequency: configure once, but the absence is a blocker for certain users.

### 3.9 Edit Clip Before Pasting — OCCASIONAL BUT MEMORABLE

Raycast lets me click into a history item and edit it before pasting. This is useful when I have a URL with a query string I need to modify, or a command where I need to change one argument before running it. The Details modal in CopyPaste shows text in a `<pre>` block that is selectable but read-only — the copy button always pastes the original. An "Edit & paste" action (opens an edit modal, user modifies, pastes the modified version — without overwriting the stored item) would cover this.

Frequency: 2–3 times per week.

### 3.10 Collections / Pinboards — MEDIUM TERM

My flat list of pinned items is currently 12 entries and already hard to scan. At 30 entries it will be unusable. I need named groups: "DevOps commands", "SQL templates", "API test payloads", "Meeting boilerplate". Paste calls these Pinboards. PastePal has Collections. This is a real organizational need as the pinned section grows beyond a screenful.

Frequency: as a setup/curation concern rather than daily flow.

---

## 4. Would I Switch to This from My Current Tool? Honest Verdict.

**Partial yes, not full yes.**

I would immediately switch from Maccy. CopyPaste is strictly better than Maccy: E2E encryption, Android sync, better search infrastructure (when FTS is wired), richer content classification, sensitive-item auto-wipe. Maccy is a fine tool but it is a single-Mac free-standing clipboard manager with no security story and no mobile sync. CopyPaste beats it on everything I care about.

**I would not switch away from Raycast** as my primary launcher-plus-clipboard tool right now, for two reasons: Raycast snippets with keyword expansion, and paste-as-plain-text. These two features alone represent maybe 40% of my productive clipboard interactions. Raycast is not going anywhere for me until CopyPaste ships both.

The security differentiator is real and meaningful to me. CopyPaste is the only tool that gives me encrypted clipboard history with Android sync and a self-hostable relay. That combination does not exist anywhere else. If snippets and paste-as-plain-text shipped tomorrow, I would cancel my Raycast subscription.

**The main workflow blockers for full adoption:**

1. No snippets/templates with keyword expansion — I rely on this every day.
2. No per-paste plain-text stripping — I paste from web to terminal constantly.
3. FTS not wired to UI — search is broken beyond 1,000 items.
4. No direct hotkeys for pinned items — too many keystrokes to get to frequently-used content.

None of these are architectural impossibilities. Items 2, 3, and 4 look like they could ship in a sprint each. Item 1 is a month-plus effort but it is the most important one.

**What I would use CopyPaste for today:** as the E2E-encrypted cross-device sync layer that I trust more than any competitor. I would run it alongside Raycast for its sync story and history, while using Raycast's snippet system. That is not ideal but it is honest.

---

## 5. My Top 10 Wishlist (Ranked)

| Rank | Feature | Why |
|------|---------|-----|
| 1 | **Snippets / templates with keyword expansion** | The #1 reason I cannot drop Raycast; covers 30+ reusable fragments I use daily; nothing else fills this |
| 2 | **Per-paste "paste as plain text" action in popup (Option+Enter or similar)** | Daily friction with HTML from web pages; global toggle is wrong model; this is table-stakes for every developer who copies from a browser |
| 3 | **FTS wired to UI search (IPC call, not client-side substring)** | The daemon already has it; search currently silently fails on items beyond page 1,000; feels like a bug not a feature gap |
| 4 | **Direct global hotkeys for pinned items** | Zero-UI access to top 5 pins; removes the popup from the hot path for truly fixed content |
| 5 | **Text transforms at paste time** | JSON prettify, base64 decode, strip HTML, URL encode — eliminates constant terminal round-trips; could be a small built-in palette to start |
| 6 | **QR pairing carries full provisioning** | Currently QR only provisions P2P; adding Supabase/relay credentials to the QR payload means zero manual entry on the phone |
| 7 | **Merge/append clips (Cmd+C+C)** | Alfred habit that I miss constantly; collecting log fragments or code snippets from multiple sources without switching to an editor |
| 8 | **MCP server for AI assistant integration** | Clipboard history as context for Claude/Cursor; Paste already shipped this; it is a thin shim over the existing IPC |
| 9 | **Undo for single-item delete (3s toast)** | Soft-delete tombstone infrastructure already exists; irreversible silent delete on a fast-moving history is too dangerous |
| 10 | **Edit clip before pasting** | Raycast habit; modify a URL query string or a command argument inline before pasting, without overwriting the stored item |
