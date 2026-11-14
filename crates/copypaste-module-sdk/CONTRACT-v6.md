# Module contract v6

Manifest schema 6 lets an optional module read history, write the clipboard
and library, keep synced and local state, ask the user, render richer results,
and serve long-lived local connections. AI tools (`modules/ai-tools`) are the
first consumer. The Rust types in this crate are the source of truth; this
document describes their semantics.

## Compatibility

- `MANIFEST_VERSION` is 6. Schema 1–5 manifests validate and load unchanged.
  Every schema 6 key (`capabilities`, `endpoints`, `localizations`, command
  `hidden`/`app_only`, field `synced`, field kinds `device_set` and
  `source_app_set`) is rejected below schema 6 and omitted when serialized at
  its default, so v5 JSON round-trips byte-compatibly.
- `ModuleInvocation` and `ModuleEnvironment` are unchanged. Hosts must not add
  fields to them: earlier SDKs decode both with `deny_unknown_fields`.
- `ModuleOutput::View` is new; only schema 6 modules return it.

## ABI decision

`MODULE_API_VERSION` stays 1. `NativeApi`, `NativeHostApi` and
`NativeHostApiV2` keep their exact layouts.

- **Module → host** requests use the existing `NativeHostApi.request`
  callback. `ModuleHostRequest` is a JSON object tagged by `operation`, like
  the sync provider requests already carried there. `HostClient::call`
  validates limits, sends the request, and decodes either the typed reply or a
  `HostError` error body. Any error body that is not a `HostError` reads as
  `Unavailable`, and earlier SDKs read every error as unavailable, so typed
  errors are invisible to v5 modules.
- **Host → module** endpoint traffic uses one new optional export,
  `copypaste_module_deliver_v1(context, data, len) -> NativeReply`, emitted by
  `export_host_module!` and dispatched to `Module::deliver` (default: refuses).
  Its input is a `ModuleDelivery` of at most `MAX_DELIVERY_BYTES`. The reply
  is released with `NativeApi.release`. A host resolves the symbol only for
  manifests that declare endpoints and fails loading when it is missing.
  `deliver` and `invoke` are serialized on one instance.
- Capability modules need the host API, so they export with
  `export_host_module!`. The host attaches the bridge to every module that
  declares a sync provider **or** any capability.
- Host error replies for requests the host cannot parse as a sync request are
  now `HostError { code: unsupported }`. That is the stub every schema 6
  service returns until a host implements it.

## Manifest additions

```json
{
  "schema_version": 6,
  "capabilities": ["history_read", "clipboard_write", "prompts", "endpoints"],
  "endpoints": [{ "id": "mcp", "platforms": ["macos", "windows", "linux"] }],
  "commands": [
    { "id": "tool", "title": "…", "description": "…", "hidden": true },
    { "id": "grants", "title": "…", "description": "…", "app_only": true }
  ],
  "preferences": [
    { "id": "excluded-devices", "title": "Devices", "kind": "device_set", "synced": true },
    { "id": "excluded-apps", "title": "Apps", "kind": "source_app_set" }
  ],
  "localizations": { "uk": { "title": "…", "commands/tool/title": "…" } }
}
```

Validation (`ModuleManifest::validate`):

- `capabilities` are unique (`history_read`, `clipboard_write`,
  `library_write`, `snippets`, `synced_records`, `local_state`, `prompts`,
  `endpoints`).
- `endpoints` is non-empty if and only if `endpoints` is declared. There are at
  most `MAX_ENDPOINTS`, with unique valid ids. Each endpoint's `platforms` is
  non-empty, unique, and a subset of `supported_platforms`.
- `synced: true` is allowed only on preferences. Secret preferences remain
  forbidden.
- `device_set` and `source_app_set` values are arrays of unique non-empty ids
  (≤ 256 bytes each, ≤ `MAX_SET_MEMBERS`) and default to `[]`. The host renders
  device and application pickers with names and icons, never raw ids.
- `hidden` commands do not appear in UI or CLI command lists (the
  `InstalledModule.commands` summary already filters them) but stay invocable.
  `app_only` commands are refused unless the caller is the CopyPaste app.

## Capability confirmation

`InstalledModule` now lists `capabilities` and `endpoints`. To enable a module,
the UI shows the declared capabilities and sends
`SetEnabled { id, enabled: true, capabilities }` with every declared capability
the user confirmed. The host refuses the request when any is missing
(`ModuleManager::set_enabled`, atomically with the change). The host persists
the confirmed set and leaves an update that declares new capabilities disabled
until the user confirms again. Every host request is checked against
`ModuleHostRequest::required_capabilities()`; missing capabilities produce
`HostErrorCode::Denied`.

## Host requests (`ModuleHostRequest`)

| operation | capabilities | reply |
|---|---|---|
| `context` | — | `ModuleHostContext { locale }` |
| `history_query` | history_read | `HistoryPage` |
| `history_get` | history_read | `HistoryItem` (`not_found` for hidden and unknown ids alike) |
| `history_facets` | history_read | `HistoryFacets` within the scope |
| `device_names` | history_read | `Vec<DeviceName>` |
| `snippet_search` / `snippet_get` / `snippet_render` | snippets | `Vec<SnippetSummary>` / `Snippet` / `RenderedSnippet` |
| `snippet_save` | snippets + library_write | `SavedRecord` |
| `set_pinned` | history_read + library_write | `null` |
| `clipboard_write` (text) | clipboard_write | `null` |
| `clipboard_write` (item) | clipboard_write + history_read | `null` |
| `record_list` / `record_get` | synced_records | `RecordPage` / `Option<ModuleRecord>` |
| `record_put` / `record_delete` | synced_records | `null` |
| `read_state` / `write_state` / `clear_state` | local_state | `{key: value \| null}` / `null` / `null` |
| `prompt` | prompts | `PromptAnswer` |
| `endpoint_send` / `endpoint_close` | endpoints | `null` |

Every history request carries a module-side `HistoryScope`:
`include_other_devices`, `excluded_devices`, `excluded_source_apps`,
`content_kinds` (an empty list matches nothing), and `lookback_days` (pinned
items are exempt; `None` means all time). Before applying the scope, the host
**always** hides items that are deleted, expired, secret, transient,
credentials, hidden from integrations (the renamed `hidden_from_ai` note and
local-rule flags), or rule-secret. `HistoryEntry.preview` holds at most
`HISTORY_PREVIEW_CHARS` characters, and `HistoryContent::Text` returns
`HISTORY_TEXT_CHUNK_CHARS` characters per call. The host does not redact
substrings; content-level redaction belongs to the module. The scope filters
`source_app` and `device` take ids from `history_facets`.

`clipboard_write` records the provenance `ModuleProvenance { module_id, label }`.
The host supplies `module_id`; `label` is at most `MAX_LABEL_CHARS`.

Common error codes: `unsupported`, `denied`, `invalid` (also produced
client-side by `ModuleHostRequest::validate`), `not_found`, `busy`, and
`unavailable`.

## Synced records

`ModuleRecord { module_id, key, value_json, deleted }`:

- `key` matches `valid_record_key` (≤ `MAX_RECORD_KEY_BYTES`, ASCII
  alphanumerics and `._:/-`, starting alphanumeric). The `host.` prefix is
  reserved.
- `value_json` is valid JSON of at most `MAX_RECORD_VALUE_BYTES`.
- A module may hold at most `MAX_RECORDS` live records. Pages hold at most
  `MAX_RECORD_PAGE` records, ordered by key; `after` continues a page.
  Deleted records are tombstones and are not listed.
- A module sees only its own records. Sync merges each record by whole-record
  last-write-wins, with storage stamps that are not part of this type. Records
  travel end-to-end encrypted, like library records. Removing the module
  deletes its records on this device only.
- Synced preference values (`synced: true`) are stored by the host as the
  reserved record `host.preferences`, a JSON object of field values. Local
  preference values stay in the registry.

## Local encrypted state

`read_state`, `write_state` and `clear_state` have the same wire shape, limits
(`MAX_STATE_KEYS`, `MAX_STATE_VALUE_BYTES`, `valid_state_key`) and storage
namespace (`module:<id>:<key>` in the encrypted store) as the sync provider
requests. One host implementation serves both. Sync providers keep implicit
access; other modules need `local_state`. State is never synced or restored,
and it is cleared when the module is removed.

## Endpoints

1. A local client (CLI) sends `Method::Modules { operation: Connect { id, endpoint } }`.
   `Connect` is not privileged. The host refuses it when the module is
   disabled, does not declare the endpoint for this platform, or already has
   `MAX_ENDPOINT_CONNECTIONS` connections.
2. The host replies once with `ResponseData::Modules { json }`, where `json`
   is an `EndpointConnected { connection }`. The IPC connection then carries
   only `EndpointFrame` JSON values (`data` or `close`), one per transport
   message, in both directions. Like `Watch`, the stream is exempt from the
   idle read deadline and counts against its own small cap.
3. The host delivers `EndpointOpen { endpoint, connection, privileged }`,
   then an `EndpointFrame { connection, data }` for each client frame. The
   module answers with `endpoint_send` from `deliver` or from its own worker
   thread. `endpoint_send` never waits on the module instance; it returns
   `busy` when the bounded outbound queue is full. `data` is at most
   `MAX_ENDPOINT_FRAME_BYTES`.
4. A client hangup, `endpoint_close`, or disabling, removing or updating the
   module ends the connection. The host sends `EndpointFrame::Close` to the
   client when it can and always delivers `EndpointClose` to the module.
5. **Keep-loaded:** a module instance with any open connection is exempt from
   idle eviction. After its last connection closes, the ordinary idle policy
   applies again. The module owns per-connection state and releases it on
   `EndpointClose`.

Endpoint authentication (for example, AI grant credentials) is part of the
module's own frame protocol. Android has no IPC endpoint and returns
`unsupported` for `Connect`.

## Prompts

`prompt` takes a `PromptRequest { title, body, choices (1–4, unique ids,
optional destructive), timeout_secs (1–300) }` and blocks the calling thread
until the user answers or the prompt expires. It returns
`PromptAnswer { choice: Option<id> }`, where `None` means dismissed, expired,
or module disabled. Prompts block the instance when called from
`invoke`/`deliver`, so modules serving several connections call `prompt` from
a worker thread. A module may have at most `MAX_PENDING_PROMPTS` pending
prompts; more return `busy`. The UI lists pending prompts with
`ModuleOperation::Prompts` (`Vec<ModulePrompt>`) and answers with
`AnswerPrompt { prompt_id, choice }`. The host emits the event kind named
`MODULE_PROMPT_EVENT` (`module_prompt`, which is `EventKind::ModulePrompt`)
whenever the pending set changes; its `module_prompts` field is the pending
count, and the prompt text is read only through the privileged `Prompts`.

## Views

`ModuleOutput::View { view: ModuleView { sections: [ViewSection { title, items }] } }`
has two item kinds:

- `row { title, subtitle, actions }`: up to `MAX_ROW_ACTIONS`
  `ViewAction { title, command, arguments, destructive }`. An action invokes a
  command of the same module with those arguments. Destructive actions are
  confirmed by the host. A `View` result replaces the current view; any other
  result is shown, and the command that produced the view runs again.
- `block { label, text }`: monospace text with a host copy button
  (≤ `MAX_VIEW_BLOCK_BYTES`).

`ModuleView::validate` enforces the shape limits. The host also checks each
action's command against the manifest. The UI uses shared shadcn components
and the existing drawer contract, with actions in a fixed footer.

## Translated strings

`localizations` maps a locale tag (`uk`, `pt-BR`; at most `MAX_LOCALES`) to
`key → translation`. Keys name existing strings: `title`, `description`,
`commands/<id>/title`, `commands/<id>/description`,
`commands/<id>/arguments/<field>/title`, `preferences/<field>/title`, and
`…/<field>/options/<choice>/title`. Unknown keys and over-long or blank titles
are rejected. `ModuleManifest::localized(locale)` resolves the exact tag, then
shorter prefixes, then the English fields. The app reports its locale with
`ModuleOperation::SetLocale { locale }` (privileged). The host localizes
`InstalledModule` summaries and returns the locale through `context`, so
modules can localize their own runtime strings (views, prompts, frames).

## Management operations

`ModuleOperation` gains `Connect`, `SetLocale`, `Prompts`, `AnswerPrompt` and
`SetEnabled.capabilities`. `ModuleOperation::privileged()` is true for
everything except `List`, `Invoke` and `Connect`; `Method::privileged()` in
`copypaste-ipc` should delegate to it. The host must also refuse `app_only`
commands to callers that are not the app; `ModuleHost::request` takes the
caller (`ModuleCaller`). The application services behind history, snippets,
pins, clipboard writes and records are the `ModuleHostServices` trait in
`copypaste-modules`; until a host implements a method it answers
`unsupported`. `local_state` uses the same store as the sync provider state.
