# Module contract v7

Manifest schema 7 lets a module own a whole library: it keeps the data in
synced records with host search, appears in the app's own surfaces (a Settings
section, Quick Paste, the Android keyboard panel and widget, History menus,
the desktop tray), builds editors from forms, and hands text to the host for
delivery. Snippets (`modules/snippets`) are the first consumer. The Rust types
in this crate are the source of truth; this document describes their
semantics. Everything in `CONTRACT-v6.md` still applies.

## Compatibility

- `MANIFEST_VERSION` is 7. Schema 1–6 manifests validate and load unchanged.
  Every schema 7 key (`storage`, `surfaces`, `provides`, text field
  `multiline`/`template`/`max_bytes`, field kinds `id_list` and `values`) is
  rejected below schema 7 and omitted when serialized at its default.
- `MODULE_API_VERSION` stays 1 and the native ABI is unchanged.
- New `ModuleOutput` kinds (`results`, `deliver`, `save_file`), view items and
  host requests are only produced or served for schema 7 modules.
- New optional fields on `ViewSection`, `ViewItem::Row` and `ViewAction` are
  omitted at their defaults, so v6 views round-trip byte-compatibly. Rust code
  builds them with `ViewSection::new`, `ViewItem::row` and `ViewAction::new`.

## Storage

```json
"storage": {
  "max_records": 40000,
  "max_value_bytes": 524288,
  "search_fields": ["title", "abbreviation", "body"],
  "keep_conflicts": true
}
```

- Requires the `synced_records` capability. It replaces `MAX_RECORDS` and
  `MAX_RECORD_VALUE_BYTES` for this module with the declared values, at most
  `MAX_STORAGE_RECORDS` and `MAX_STORAGE_VALUE_BYTES`.
  `ModuleHostRequest::validate` checks `record_put` against the ceiling; the
  host enforces the declared limit.
- `search_fields` (≤ `MAX_SEARCH_FIELDS`) name top-level string fields of
  record values. The host keeps a local full-text index of them;
  `record_search { query, prefix, limit }` returns live records ranked by
  relevance, at most `MAX_SEARCH_PAGE`. Ranking beyond relevance (exact
  abbreviations, pins) belongs to the module.
- With `keep_conflicts`, when sync replaces a local value with a remote one,
  the host retains the replaced value (at most `MAX_RETAINED_CONFLICTS` per
  module, oldest dropped first). `record_conflicts { limit }` lists them as
  `ReplacedRecord { id, key, value_json }`; `clear_record_conflicts { ids }`
  forgets them. Retained values are local and never synced. The module decides
  whether a replaced value is a real conflict, for example by comparing
  revisions it stores in the value.
- Records keep syncing when the module is not installed on a device, as in
  v6, so a library is never lost on devices without the module.

## Surfaces

```json
"surfaces": {
  "settings": { "command": "library", "icon": "notebook_text" },
  "quick_paste": { "title": "Snippets", "search_command": "search",
                   "activate_command": "use", "mixed_limit": 5 },
  "item_actions": [{ "id": "save", "title": "Save as snippet",
                     "command": "save-item", "icon": "file_plus" }],
  "tray_actions": [{ "id": "new", "title": "New snippet", "command": "new" }]
}
```

Surfaces appear only while the module is installed, enabled and loaded on
this device. Each command must exist with exactly the text arguments listed:

| Surface | Command arguments | Output |
|---|---|---|
| `settings` | none | `view`; a top-level Settings section titled like the module |
| `quick_paste.search_command` | `query` | `results`; an empty query lists the tab |
| `quick_paste.activate_command` | `id` | `deliver`, or a `view` whose form submit returns `deliver` |
| `item_actions[].command` | `item_id`, `text` | a `view` opened in a drawer, or a message |
| `tray_actions[].command` | none | a `view` opened in a drawer over the settings section |

- `quick_paste` is a Quick Paste tab, the Android keyboard panel tab and the
  widget filter, all labeled `title`. With a non-empty query, up to
  `mixed_limit` (≤ `MAX_MIXED_RESULTS`) results also appear above history.
  The host adds its own "Copy" and "Add to Paste Queue" actions; they run the
  activate command and route the delivery.
- `item_actions` (≤ `MAX_ITEM_ACTIONS`) are offered for text history items.
  The host passes the item text it already shows the user.
- `tray_actions` (≤ `MAX_TRAY_ACTIONS`) are desktop only.
- Icons are `ModuleIcon` names the host maps to its own icon set.
- Localization keys: `surfaces/quick_paste/title`,
  `surfaces/item_actions/<id>/title`, `surfaces/tray_actions/<id>/title`.

## Service providers

`"provides": [{ "service": "snippets", "command": "serve" }]` makes this
module answer `snippet_search`, `snippet_get`, `snippet_render` and
`snippet_save` for other modules holding the `snippets` capability. The
command must be `hidden` and take one required text argument, `request`: the
`ModuleHostRequest` as JSON. It returns `data` holding a `ServiceReply`
(`{"status": "ok", "reply": …}` or `{"status": "error", "error": HostError}`).
The provider omits concealed items itself; the host still applies its secret
redaction to replies. Without an enabled provider these requests answer
`unavailable`: the app keeps no snippet library of its own. Each service has at most one provider per module, and
the host uses the first enabled one.

The `snippets` provider may also call `legacy_snippets { after, limit }` once
to move the library the app kept before snippets became a module. It returns
`LegacySnippetPage { collections, snippets, next }` with collections on the
first page only, purged snippets omitted, and local usage counts included.
The host refuses the request to any other module.

## Views

Schema 7 adds to `ModuleView`:

- `ViewSection.bulk_actions`: actions on selected rows; the host passes the
  selected row ids as `ids` (an `id_list` argument).
- `ViewSection.reorder`: runs after a drag with `id` and `before`, the id of
  the next row or `""` at the end.
- `ViewSection.more`: loads more rows, replacing the view.
- `row` gains `id` (needed for selection and reordering), `badges`
  (≤ `MAX_BADGES`), `secret` (masked subtitle), `icon`, and `open`, the action
  for activating the row itself.
- `search { query, placeholder, action }`: the host runs `action` with `query`
  after edits.
- `filter { options, selected, action }`: one choice among options with
  optional counts and icons, such as a scope or a collection; picking one runs
  `action` with `value`.
- `toolbar { actions }`: up to `MAX_TOOLBAR_ACTIONS` buttons.
- `notice { text, tone }`: `info`, `warning` or `error`.
- `form { fields, values, errors, submit, preview }`: `fields` are
  `ModuleField`s (text, choices, boolean, file). `values` pre-fill them,
  `errors` are messages by field id. `submit` runs with the field values as
  `values` (a `values` argument). `preview`, when present, runs with the same
  argument after edits; its `text` or `message` output is shown below the
  fields.
- `preference { id }`: one of the module's preferences, rendered and saved by
  the host as in the module settings drawer.
- `ViewAction.target`: `replace` (default) or `drawer`. A drawer closes when an
  action inside it returns anything but a view, and the view under it runs
  again.
- `ViewAction.require_auth`: the host authenticates the device owner first.

`ModuleView::commands()` lists every command a view can run, so the host can
check them against the manifest.

## Fields

- `text` gains `multiline`, `template` (the host highlights `{{…}}`
  placeholders), and `max_bytes` (default `DEFAULT_TEXT_BYTES`, at most
  `MAX_STORAGE_VALUE_BYTES`).
- `id_list` (≤ `MAX_ID_LIST` unique ids) is filled by the host from a
  selection and never shown as an input.
- `values` is an object of at most `MAX_FORM_VALUES` entries holding strings,
  booleans or string arrays, keyed by form field id.
- Neither new kind can be a preference.
- `choices` may hold up to `MAX_CHOICE_OPTIONS` options and a `filter` up to
  `MAX_FILTER_OPTIONS`. File `accepted_extensions` may contain `-` and be up
  to 32 characters (for example `copypaste-snippets`).

## Outputs

- `results { items }`: `ResultItem { id, title, preview, badges, secret,
  preselect, actions }`, at most `MAX_RESULTS`, at most one `preselect`.
  `secret` masks the preview.
- `deliver { delivery }`: `Delivery { text, secret, require_auth,
  record_history, label, cursor_graphemes_from_end }`. The host pastes, inserts, copies or queues the
  text depending on where the user activated it. With `require_auth` it
  authenticates first. Secret text is written as a secret and never recorded;
  otherwise `record_history` records it with the provenance label.
  `cursor_graphemes_from_end` places the caret after an insertion. Hosts
  queue a delivery as a stored copy that is never captured in history
  (`QueueSource::Delivered`), masked when secret.
- `save_file { path, name }`: a file the module wrote under its data
  directory. The host asks the user where to save it as `name`, copies it
  there, and deletes the original.

`ModuleOutput::validate` checks these shapes; hosts call it before rendering.

## Capabilities and host requests

| operation | capabilities | reply |
|---|---|---|
| `record_search` | synced_records | `RecordPage` (`next` is `None`) |
| `record_conflicts` | synced_records | `Vec<ReplacedRecord>` |
| `clear_record_conflicts` | synced_records | `null` |
| `clipboard_read` | clipboard_read | `ClipboardText { text }` |
| `legacy_snippets` | snippets + synced_records, `snippets` provider only | `LegacySnippetPage` |

`clipboard_read` is a new capability. It returns the text the user last
copied (CopyPaste's own pending deliveries are skipped), or `None` when it is
not text. Unlike other replies the text is not redacted: the user granted the
capability for exactly this. `secret` marks secret text, and anything a module
derives from it must be delivered with `secret: true`.
