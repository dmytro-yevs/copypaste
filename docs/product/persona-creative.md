# CopyPaste — Creative / Knowledge-Worker Persona Critique

**Persona:** Maya, freelance UX designer + researcher. Also a technical writer who freelances for two SaaS companies. Works on a MacBook Pro (main) and a Pixel 8 Pro (secondary, Android). Daily workflow: browser research, Figma, Notion, VS Code for Markdown, Slack. During a session she copies 80–120 things — screenshots of competitor UIs, hex color values, reference URLs, code snippets, formatted text from PDFs, image assets. She reuses clips hours later and across sessions. She has paid for Paste ($30/yr), Pastebot ($13), and PastePal ($30 lifetime), and has strong opinions about what "a good clipboard manager looks like."

---

## 1. Visual & Content Handling

### What I actually tried to do

I copied a screenshot of a competitor's UI, a color swatch from Figma (HTML clipboard includes the hex value), some rich text from a Notion doc (bullet list, bolded headings), a PDF snippet, a URL, and a code block from VS Code. Then I opened CopyPaste.

### Images: functional but aesthetically thin

The thumbnail display works. I can see my screenshots in the history list. The `imageMaxHeight` slider goes up to 200px, and with it cranked up the images are actually usable previews — this is better than Maccy's fixed tiny thumbnails. The details modal shows full-resolution at up to 600px height, which is fine for a quick visual confirm.

But the thumbnail is generated at capture time with a fixed 192px maximum dimension on the longest side. That is small. On a 27" monitor at 2x resolution, a 192px thumbnail renders soft and indistinct. If I copied a dense UI screenshot to compare with another design, the in-list preview is not sharp enough to be useful — I have to open the details modal every time. Compare this to Paste's Pinboards view which shows large, crisp previews that I can actually read. Or PastePal's grid view with configurable card sizes.

The `imageMaxHeight` slider only controls display height in the row, not the underlying thumbnail resolution. So cranking the slider to 200px just stretches a 192px PNG — it goes blurry. That distinction is invisible to users and the result is frustrating.

There is also no hover preview. Maccy added hover image expansion. CopyPaste makes me open a modal every time I want a closer look. For image-heavy workflows that is an extra click per inspection, which adds up.

### Rich text and HTML: simply gone

This is where I gave up as a creative professional. When I copy styled text from Notion — a bulleted list with a bold header — CopyPaste captures the plain-text representation. The rich formatting is stripped entirely. `public.rtf` and `public.html` clipboard types are logged and discarded by the daemon. I know this because the docs are candid about it: "Unsupported: public.rtf, public.html — Logged once per kind; never captured."

So when I copy a formatted table from a Notion page into CopyPaste, then paste it into a Google Doc an hour later, I get unformatted plain text. The carefully maintained structure — column widths, bold labels, indentation — is gone. Paste, Pastebot, PastePal, Alfred, Raycast, Ditto, and even ClipboardFusion all preserve RTF and HTML. This is not a minor gap. For a writer or designer who copies formatted content from web pages, docs, or spreadsheets many times per day, losing formatting on every reuse is a constant tax.

The daemon does have a `paste_as_plain_text` config option, but that goes the wrong direction — it strips formatting intentionally. What Maya needs is the opposite: preserve formatting so she can choose to use it or strip it. The choice should be at paste time, not at capture time.

### Files: workable but bare

File clips show a filename, MIME type, Save As and Copy buttons. That is correct and functional. What is missing: no inline preview of file content (no text preview for `.md` or `.txt` files, no PDF first-page thumbnail, no image preview for `.svg`). Everything is just a filename chip. When I copy-collect several design assets into CopyPaste, I have no way to tell them apart visually except by filename.

---

## 2. Organization

### What exists: pinning and a flat list

The only organizational tool available is pinning. I can pin items and drag-reorder them within the pinned section. That is a genuine differentiator over Maccy (no drag reorder) and Raycast (no drag reorder for clipboard), and I appreciate it.

But the moment I have more than 10–15 pinned items, the pinned section becomes its own ungrouped mess. There are no labels, no dividers, no sub-groups. If I pin my 8 design color tokens, 3 recurring code snippets, my email signature, 2 API keys I keep for testing, and my home address for form-filling, they all sit in one undifferentiated amber-flagged pile. Finding the right one requires reading every item. The list has no visual grouping.

### What is completely absent

- **No folders, collections, or pinboards.** Paste has Pinboards (shareable, syncable). PastePal has Collections. Pastebot has Custom Pasteboards with individual hotkeys. CopyPaste has nothing analogous. A pinned flat list is not the same as a curated board.
- **No tags.** I cannot tag a clip "design-ref" or "client-a" to filter later.
- **No snippets / template store.** The competitive gap analysis correctly calls this out as the #1 unimplemented feature by power-user impact. Reusable boilerplate (email templates, standard disclaimers, code patterns) live nowhere in CopyPaste. I keep them in a separate Raycast Snippets collection. If CopyPaste wants to replace Raycast for me, it needs a snippets layer.
- **No favorites distinct from pinned history items.** Pins apply to ephemeral history items; they are purged when the item ages out (unless pinned items exempt — which they are, forever, but that means my pin list grows unboundedly and never gets curated).

The organizational model is: reverse-chronological list with a pinned subsection. That is adequate for a lightweight clipboard watcher (Maccy). For a creative professional collecting and reusing content across projects and sessions, it falls short of what Paste or PastePal deliver.

---

## 3. Finding Things Later

### Search: good for text, blind to images

The FTS5 full-text search is real and indexed — the daemon builds a proper search index over clipboard text content. That is better than Maccy (client-side substring over in-memory list) and puts CopyPaste on par with Paste. For text-heavy workflows the search is fast and accurate.

But every image in my history is a black box to search. If I copy a screenshot of a server error message, I cannot search for the error text inside it. Raycast added Vision-framework OCR for this. Paste added it via Apple Intelligence in late 2025. CopyPaste captures images but stores only the pixel data — no text extraction, no indexing of image content. For a researcher who copies screenshots of papers, dashboards, or competitor UIs, this means images are effectively unfindable by content. I have to remember approximately when I copied a screenshot and scroll to find it.

### No filtering by content type

The filter toolbar has a device filter and a sort-mode toggle. There is no "show only images" or "show only files" or "show only URLs" filter. When I want to find a specific screenshot among 200 text clips, I scroll. Paste, PastePal, and Pastebot all offer type-based filtering as a first-class feature. This is a significant QoL gap for mixed-content workflows.

### Search scope is limited to the loaded page

The client-side search in the main window operates over the loaded page (up to 1,000 items) — but load-more is disabled while a search is active, so items beyond the first page are effectively invisible to search. In practice the FTS daemon query is available, but the UI presents client-side search for the history view. This means if I copied something 1,200 items ago, a search will not find it. For someone who runs CopyPaste for weeks without clearing, history gaps in search are a real problem.

---

## 4. Aesthetics and Feel

### macOS: close to frame-worthy, not quite there

The Tauri-based UI is genuinely attractive — NSVisualEffectMaterial vibrancy, clean token-based design system, smooth animations, proper macOS chrome. The popup feels native. The sidebar navigation is coherent. This is meaningfully better than most Tauri/Electron clipboard managers I have tried.

Where it falls short of "frame-worthy":

- The KindChip text is rendered at 9px — below the design system's own 10.5px floor. It is illegible at normal viewing distance. Not a big deal for text classification, but a visible roughness.
- Image thumbnails are 192px underlying resolution regardless of what the display slider says. Stretching them to 200px display height makes them visibly blurry. An app I pay for should show crisp images.
- The main history window and the popup use different chip styles for the same content type (full-word label vs single-character glyph). This inconsistency within a single platform is the kind of thing designers notice.
- The details modal is a bottom-anchored overlay, not a proper centered panel. It reads as "added later" rather than designed as part of the experience.
- There is no image hover-expand or quick-look gesture — I have to commit to opening a modal to see full resolution.

Paste ($30/yr) genuinely feels more polished: larger thumbnails, proper Pinboard card layout, smooth spring physics, beautiful empty states. PastePal feels more coherent in its visual hierarchy. CopyPaste is not embarrassing, but it is not the visual peer of those apps yet.

### Android: rough around the edges

The Android app is functionally solid but visually inconsistent with the macOS app and with its own design system. Section labels are 16sp `titleMedium` instead of the spec's 11sp. The top bar is 22sp instead of 13px. Material filled icons are used everywhere while the design system specifies Lucide stroke icons. For a designer these details are not cosmetic — they signal a product that has not been finished.

The history row is dense and functional but lacks the polished micro-interactions of the macOS app. The press-scale animation is there, the pinned amber left-border is there, but the overall feel is a mid-fidelity implementation. Compared to a polished Android app like Notion or Linear, CopyPaste's Android UI looks like a developer-built prototype, not a shipped product.

---

## 5. What Is Missing for My Workflow (Prioritized)

### P0 — Blockers that prevent adoption as my primary tool

**1. No rich-text / HTML preservation.** Every formatted copy is degraded to plain text. This is a daily friction point for any knowledge worker. Without it, CopyPaste cannot replace Paste or Pastebot for formatted-content workflows.

**2. No OCR / image text search.** Screenshots are dead weight in search. Given that both Raycast and Paste now ship this, it is becoming table-stakes. I cannot rely on a clipboard manager that can't find content in my own screenshots.

**3. No collections / pinboards / folders.** Flat pinned list does not scale beyond ~10 items for curation. Without named collections, CopyPaste cannot store my design token palettes, client-specific boilerplate, or reference material in any organized way.

### P1 — Significant friction, would accept with workarounds

**4. No snippet library.** I need a place to store reusable templates that persist independently of clipboard history turnover. Currently I maintain a Raycast Snippets collection for this. If CopyPaste had snippets, I could consolidate.

**5. No content-type filter.** "Show me only images" or "show me only URLs" from today's session is a basic workflow. Scrolling through 200 mixed items to find a specific screenshot is tedious.

**6. No hover quick-look for images.** Every image requires a modal open to see properly. A hover expand (like Maccy's) would handle 80% of quick-confirm needs without navigating away.

**7. No paste-format choice at paste time.** Global "paste as plain text" config is not the same as a per-paste choice. I want "paste formatted" as the default and "paste as plain text" as an alt-action — not a global toggle.

**8. Thumbnail resolution too low.** 192px maximum dimension for a 27" display at 2x scaling is not enough. Thumbnails should be at minimum 400px on the long edge, with a configurable option. The current slider only stretches the too-small thumbnail.

### P2 — Quality-of-life gaps

**9. No drag-out of images / files to other apps.** In Paste I can drag an image from clipboard history directly into Figma or a Finder folder. CopyPaste requires me to copy the item back to the clipboard first, then paste. Drag-out is a workflow accelerator for design tools.

**10. Android image quality feels secondary.** The source-app chip, device badge, and content-type labeling on Android history rows are less polished. No `KindChip` equivalent for text subtype. Minor compared to above, but noticeable when switching between devices.

---

## 6. Verdict

CopyPaste has a genuinely strong technical foundation — E2E encryption, three-path cross-device sync, PAKE device pairing, macOS Keychain key storage. For a privacy-conscious user who syncs between Mac and Android, there is nothing comparable. The security model is the best in the category, not just marketing.

But for a creative professional, security architecture is table stakes, not a differentiator. What I reach for every hour is: "did it capture the formatted version of what I copied, can I find it later, can I organize it into something meaningful?" On all three, CopyPaste currently falls short of what I can get from Paste ($30/yr) or PastePal ($30 lifetime).

The app is halfway between a utilitarian history logger (Maccy tier) and a creative workspace (Paste tier). It needs to pick a side and execute. The design system, the attention to animation, and the image-height slider all suggest the team wants to be Paste-tier. Getting there requires — in order of importance — rich-text preservation, OCR, and collections.

---

## Top 10 Wishlist (Ranked)

| # | Feature | Why it matters |
|---|---------|---------------|
| 1 | **Rich-text / HTML preservation at capture** | Daily friction for every knowledge worker; fundamental parity gap vs. all premium competitors |
| 2 | **Collections / Pinboards (named groups of clips)** | Flat pinned list breaks down past 10 items; no curation model for project-based content |
| 3 | **OCR — index text inside images for search** | Screenshots are the fastest-growing clip type; currently 100% unsearchable by content |
| 4 | **Content-type filter in history** (images / URLs / text / files) | The single highest-frequency "find a specific item" workflow; 10 seconds vs 60 seconds |
| 5 | **Snippet / template library** (persistent, separate from history) | Boilerplate that survives history purge; currently forces a second app (Raycast/TextExpander) |
| 6 | **Larger thumbnails** (min 400px long edge, lossless or high-quality WebP) | 192px is blurry at 2x; image-heavy users need readable in-list previews |
| 7 | **Hover quick-look for images** (expand in-place, no modal required) | Eliminates the modal round-trip for 80% of "is this the right screenshot?" checks |
| 8 | **Format-choice at paste time** (formatted / plain text / per-paste) | Replace global toggle with contextual action in popup or hover menu |
| 9 | **Drag-out images/files to other apps** | Figma, Sketch, Finder drop — removes the "copy back then paste" detour |
| 10 | **"Show only images" / "Show only URLs" quick-filter** | Pairs with #4; zero new infrastructure needed if the daemon already classifies types |

---

*Persona critique based on a read-only audit of: `docs/product/features-macos.md`, `features-android.md`, `features-sync.md`, `features-core-security.md`, `competitive-gap-analysis.md`, `docs/ux/ux-ui-review.md` (branch: `v0.6.1-integration`, 2026-06-04). No code was modified.*
