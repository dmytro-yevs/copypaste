# Snippets

Snippets is an optional module for reusable text and templates. It keeps your
library in synced, end-to-end encrypted module records, so it follows you to
every paired device, including devices where the module is not installed
(there it waits until you install it).

Install it from **Settings → Modules** and confirm its permissions. The first
time it runs on a device, it moves the snippet library the app kept before
snippets became a module. A snippet already in the module is never
overwritten, so running on several devices is safe.

## Where it appears

- **Settings → Snippets**: search, filter by All, Pinned, a collection,
  Unfiled or Recently Deleted, create, edit, pin, reorder, move, delete and
  restore snippets, manage collections, import and export, and the two
  preferences below.
- **Quick Paste** (also the Android keyboard panel tab and the widget
  filter): a Snippets tab, and up to five snippet results above history while
  you type. A single exact abbreviation match is preselected. Templates with
  fields open a fill-in form first.
- **History**: **Save as snippet** opens the editor with the item's text.
- **Tray and menu bar** (desktop): **New snippet**.
- **AI Tools**: the module answers the snippet search, read, render and save
  requests of other modules. Concealed snippets are never served, and
  templates that read the clipboard are refused.

## Templates

Turn on **Template** to fill in `{{date}}`, `{{time}}`, `{{uuid}}`,
`{{cursor}}`, `{{field name="…"}}`, `{{clipboard}}`, `{{snippet abbr=…}}` and
modifiers such as `| upper`. `{{clipboard}}` (index 0) is the text you last
copied. The module does not read your history, so `{{clipboard index=N}}`
with N ≥ 1 is left empty. Renaming an abbreviation can update the templates
that insert it.

## Preferences

- **Record pasted snippets in History** (synced, off by default).
- **Verify before using concealed snippets** (synced, off by default): asks
  for your password or biometrics before a concealed snippet opens or pastes.

Concealed snippets are written to the clipboard as secrets, are never
recorded in History, and their text is not indexed for search.

## Limits

- 20,000 snippets (a warning above 10,000) and 200 collections.
- Title 200 characters, abbreviation 32 characters without spaces, text
  64 KB (256 KB with formatting), collection name 64 characters.
- Deleted snippets stay in Recently Deleted for 30 days, then they are erased
  on every device.
- Import reads CopyPaste bundles (optionally encrypted with a passphrase of at
  least 8 characters), JSON lists of `{name, text, keyword}` and CSV files with
  a body column: at most 50 MB and 10,000 snippets.
- When two devices edit the same snippet offline, the replaced version is kept
  as a conflict copy.

Usage counts that order results stay on each device.

## Platform notes

Snippets ships for macOS, Windows, Linux and Android. The tray entry is
desktop only. Export files are written as `.copypaste-snippets` bundles, the
same format and extension earlier versions used, and import also takes
`.json` and `.csv` files.

## Strings

Runtime strings come from the app's translations, kept as a snapshot in
`l10n/`. `scripts/generate-strings.py` regenerates `src/strings_table.rs`
from it.
