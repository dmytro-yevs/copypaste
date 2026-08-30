# Port Manifest 06 — UI Behaviour

This manifest specifies current v2 user-visible behaviour for the shared React
application and its macOS, Android and Windows hosts. `docs/ui-architecture.md`
owns component boundaries and composition. Generated IPC types own domain
vocabulary. This document owns observable state, security, accessibility and
failure behaviour.

There is one product UI. Platform capability data selects native actions; no
surface infers a platform, device class or permission state from a display name
or user-agent string.

## 1. Global invariants

### 1.1 Data and concurrency

- **INV-1:** Virtual-list position anchors to an item and its measured offset,
  never a raw scroll pixel.
- **INV-2:** Equal query data preserves its list reference to avoid idle
  rerenders.
- **INV-3:** A local mutation invalidates the relevant query/dedup signature.
- **INV-4:** Loading another page merges by stable item id and never replaces
  newer head data.
- **INV-5:** Initial row reservations are conservative; rendered rows replace
  estimates through measurement.
- **INV-6:** Shrinking content clamps scroll position into the new range.
- **INV-7:** Active-row references name only mounted elements.
- **INV-8:** Selection is tracked by item id, not array index.
- **INV-9:** Async responses are sequence-tagged or cancelled so an earlier
  response cannot overwrite a later one.
- **INV-10:** Polling is visibility-gated and uses shared backoff.
- **INV-11:** Optimistic writes revert the affected field or collection on
  failure.
- **INV-12:** Busy state is released on success, error and cancellation.
- **INV-13:** Pinned items keep their explicit order when copied.
- **INV-14:** A full-body read failure is an explicit unavailable state. A
  truncated list preview is never presented as the complete body.

### 1.2 Sensitive data and errors

- **INV-15:** Masked content is absent from DOM text, accessible names, search
  targets and shared caches. Blur alone is not masking.
- **INV-16:** Revealed plaintext is ephemeral component state, never React Query
  data, a global store, diagnostic event or log. It re-hides after the configured
  timeout and on visibility loss.
- **INV-17:** The pairing QR payload never enters the DOM. A protected native
  renderer receives it through the bounded host contract.
- **INV-18:** SAS digits are inert display text. The app provides explicit
  Accept and Reject controls and never submits on digit interaction.
- **INV-19:** Peer-advertised name and platform beside SAS are labelled
  unverified until the ceremony succeeds.
- **INV-20:** User-visible errors are selected from typed codes and safe
  presentation data. Raw platform/network errors, filesystem paths, usernames,
  tokens and backend response bodies are never rendered.
- **INV-21:** Screen-capture protection is enabled by default on every native
  window that can display clipboard or pairing content.
- **INV-22:** Unknown content classes and error codes remain unknown; the UI does
  not guess that they are text, files or retryable failures.

### 1.3 Focus, windows and global state

- **INV-23:** Shell chrome remains outside route error boundaries so navigation
  and recovery survive a feature crash.
- **INV-24:** One alert banner is visible at a time according to explicit
  priority. Informational status does not displace a higher-severity alert.
- **INV-25:** Only one confirmation dialog may be active at a time.
- **INV-26:** Body scroll locking is reference-counted across nested dialogs.
- **INV-27:** Closing the main window hides it; only the explicit Quit action
  exits the application.
- **INV-28:** Hiding Quick Paste returns focus to the application that was active
  before it opened. With no target, the host uses a non-focusing hide path.
- **INV-29:** Copy completes before a window hides. Failure keeps the surface
  open and shows a safe error.
- **INV-30:** Blocking IPC and filesystem work never runs on the UI/main thread.
- **INV-31:** Private mode converges across app, tray and platform surfaces from
  daemon-confirmed state. A failed toggle reverts.
- **INV-32:** Appearance preferences are applied before first paint and then
  synchronized live.
- **INV-33:** Persisted preference corruption falls back per field, preserving
  every other valid setting.

## 2. Accessibility contract

### 2.1 Lists and selection

History is a labelled `role="list"`; rows are `role="listitem"` with
`aria-current` for selection. Rows contain buttons, so listbox/option semantics
are forbidden. Keyboard selection is announced through a separate polite live
region. Any debug active-descendant id is cleared when its row unmounts.

Arrow keys move within bounds without wrapping, Home/End move to the ends,
Enter copies and Space toggles selection where that action is available.
Navigation scrolls the selected row into view. Pointer hover does not steal a
selection immediately after keyboard navigation or during scroll momentum.

### 2.2 Dialogs and controls

Dialogs portal to `document.body`, expose `role="dialog"`, `aria-modal`, a
labelled title and optional description. Focus enters the first focusable
control, remains trapped, and returns to the element that launched the dialog.
Escape and backdrop dismissal are explicit independent options.

Tabs use tablist/tab/tabpanel relationships. Arrow navigation follows
orientation, wraps, and supports Home/End. Disclosures expose `aria-expanded`
and `aria-controls`. Toggle buttons expose `aria-pressed`.

Every icon-only control has an accessible name and pointer tooltip/title;
decorative icons are hidden from assistive technology. A disabled control that
needs explanation remains discoverable through adjacent description or a
focusable wrapper.

### 2.3 Live regions and urgency

Warnings and failures use `role="alert"` only when immediate interruption is
warranted. Progress, startup and confirmations use `role="status"`. Capture
health and onboarding permission presentation each come from one exhaustive
descriptor that owns tone, label, action, disabled state and ARIA role; two
screens may not interpret the same enum independently.

Toast timers pause while hovered or focused. A transient status announces that
it will close. Live regions do not repeat unchanged messages on every poll.

### 2.4 Visual accessibility

All text meets WCAG AA contrast. Decorative hue tokens are not reused as small
foreground text unless the generated contrast gate approves them. Reduced
motion removes decorative animation without hiding state; reduced transparency
forces solid surfaces. Coarse pointers receive at least a 44px target even when
the visual control remains compact.

## 3. App shell and shared state

The shell owns navigation, route selection, global listeners, banner priority,
appearance synchronization and platform capability normalization. Feature
screens own neither process lifecycle nor global event subscriptions.

The current boundary is exposed to the UI as `CURRENT_PROTOCOL_VERSION = 2`;
the documentation gate verifies that value against the Rust protocol owner.

Required shell behaviour:

- unrecognized route/view values resolve to History;
- navigation and current route remain usable when a feature boundary fails;
- native event listeners are registered only when the native bridge exists and
  are removed on unmount;
- protocol mismatch, service startup failure and permission state are rendered
  from typed status, not process/version string parsing;
- a protocol mismatch offers restart/update guidance for the current app and
  service without probing or naming another installation;
- an incoming pairing event opens the responder flow only after validated
  pairing state is available;
- appearance changes update document and native chrome from one resolved
  scheme;
- private mode events refresh every mounted consumer.

Global banners do not expose daemon paths or raw spawn output. Dismissal never
marks an unresolved daemon failure as healthy.

## 4. History

### 4.1 Data and virtualization

History fetches a bounded first page and loads more through an opaque cursor.
Search queries the backend search operation rather than filtering loaded rows.
Grouping, filtering and sorting are React-free presentation transformations of
one canonical item model.

Cards have intrinsic height. Text and code use complete-line clamps; images use
bounded aspect reservations and `object-fit: contain`; source/state metadata
adds finite measured rows. TanStack Virtual owns measurement and replaces every
estimate with actual geometry.

Prepend, append, image load, preference change, group collapse and deletion
preserve an item anchor where one still exists. When the anchor disappears, use
the nearest surviving row and clamp.

### 4.2 Body presentation

One React-free resolver maps list preview, full-body result, full-body failure,
sensitivity, reveal state and media kind to a discriminated presentation:
masked, unavailable, image, code or text. Inspector and expanded reader consume
the same resolver.

List payloads are previews. Non-sensitive Inspector/detail views request full
content by id. Sensitive content never crosses that operation and uses the
separate reveal command only after an explicit gesture. If full content cannot
be read, both surfaces say it is unavailable.

One presentation record owns singular type labels, filter labels, icon and copy
action copy. File display name, origin display name and absolute time each have
one formatter. Device icons use generated `DeviceClass`; `unknown` remains
generic even when a display name contains a platform word.

### 4.3 Actions

- Copy, plain-text copy, pin, unpin, delete and reveal are explicit actions.
- Inspector and detail use identical copy icon/label policy for the same kind.
- Multi-select actions snapshot selected ids and release busy state in a
  `finally` path.
- Pinned reorder is optimistic and restores server order on failure.
- Delete undo is bounded and cannot delete or restore an item outside the
  captured action set.
- Delete-all passes the capture ceiling associated with the user's gesture so a
  later capture survives.
- Search and copy targets use masked presentation for sensitive items.
- Import requires confirmation before any database mutation and refreshes
  History after success.

## 5. Capture and onboarding

Capture status consumes the generated snapshot and next-step enums. The model
maps every health/next-step value exhaustively to title, detail, action, tone
and live-region urgency. The same fault has the same semantics in onboarding,
Capture and Settings.

Every onboarding permission status has an explicit policy. `not_required` is
complete and non-actionable; `granted` is complete; `prompt`, `denied` and
`unavailable` have distinct explanations and only valid actions. When settings
are the recovery path, the action opens platform settings rather than repeating
an impossible permission request.

Progress and failure states remain visible while a native request is in flight.
Cancellation releases busy state. Android tile/service instructions state their
actual runtime dependency and fail closed when the service cannot observe the
required source.

## 6. Devices and pairing

The Devices screen shows known peers, discovery state, per-peer health and
pairing actions. Presence is tri-state: online, offline or unknown. A transient
poll failure keeps the prior rows visible but marks freshness honestly; it does
not reinterpret every peer as offline.

Unpair and revoke are distinct confirmations. A dialog remains open and
actionable when the operation fails. Peer names, addresses and fingerprints are
bounded and safely formatted; missing device class uses the generic glyph.

Pairing requirements:

- Allow screenshots applies to the product shell and Quick Paste, never native
  pairing prompts. Windows pairing evidence is accessibility-only, proves
  `WDA_EXCLUDEFROMCAPTURE`, and reads no protected field value.
- QR generation happens only while its protected renderer is visible;
- hiding the modal pauses regeneration so single-use tokens are not burned;
- regenerate re-blurs/protects the new code before display;
- closing any pairing modal cancels the daemon ceremony and clears local state;
- SAS timeout removes decision buttons;
- local accept followed by a neutral terminal state is success only when the
  ceremony reports the confirmed peer;
- a responder modal is never created from an initiator event or stale local
  state;
- only one pairing/revoke confirmation can own focus at once.

## 7. Settings

Settings load independent domains concurrently and retain partial results. A
failure in one domain does not erase successfully loaded values. Service
offline, starting, key-unavailable and generic failure states have distinct
safe recovery actions.

### 7.1 Saves and credentials

- Toggle saves are optimistic with field-scoped revert.
- A patch is built from current component state so one action cannot overwrite
  another unsaved field.
- Structured feedback, not string comparison, distinguishes success/failure.
- Controls that require service restart stay busy until the confirmed restart
  result; failure clears success presentation.
- Test connection first saves and aborts if save fails.
- Credentials are write-only. Presence may be returned; secret values are not.
- Blank secret inputs preserve stored values by omission, and successful save
  clears the input.

Private mode uses the echoed daemon value and broadcasts the result. Backup and
import operate through the typed transfer contract. Restore/import requires an
affirmative confirmation and never touches the store before it.

### 7.2 Appearance and shortcuts

Appearance choices use one mode (`system|light|dark`) and one generated product
theme. System mode follows a live color-scheme query. Theme tokens, previews and
native chrome resolve from the same generated values.

Shortcut capture uses physical keys, ignores bare modifiers and Escape cancels
without changing the binding. An unregisterable accelerator reports failure and
does not crash startup. The default accelerator comes from the Rust owner. The
accessible name speaks the raw accelerator string, not only glyphs.

### 7.3 Diagnostics

Log tailing keeps one bounded head window plus a multiset merge cache so equal
lines and timestamp-sharing bursts are not lost. A stopped tail says that the
visible rows may be out of date. Exported diagnostics pass through the shared
redaction boundary.

## 8. Quick Paste, tray and notifications

Quick Paste is lazy-created, hidden by default, kept warm after first use and
clears its item/image memory on hide. Its position is calculated in physical
pixels and clamped to the selected monitor, including negative coordinates.

It records the active application before showing. The single hide path restores
that target at most once even when blur and row activation race. Search receives
focus only after the native window is ready.

Keyboard behaviour:

| Key | Result |
|---|---|
| Up/Down | Move with wrapping. |
| Enter | Copy, hide, paste. |
| Alt/Option+Enter | Copy as plain text, hide, paste. |
| Escape | Hide. |
| Command/Ctrl+1–9 | Activate the numbered item only with an empty query. |

Popup refresh occurs on show/focus, a visibility-gated interval and explicit
retry. Results are sequence-tagged. Loading an intentionally cleared cache shows
a neutral blank/loading state rather than claiming History is empty. Search uses
masked display labels and never plaintext from sensitive items.

Tray setup never blocks on IPC. Private-mode check state and recent items are
filled asynchronously from daemon truth. Recent labels are bounded and collapse
control characters. Tray copy uses the same feedback settings as other
app-owned copy actions.

Notification and sound occur only after successful copy/capture and follow
independent user settings. Toasts stack without overlap, remain below dialogs,
pause dismissal on interaction and expose a dismiss control.

## 9. Shared components and layout

The directed dependency and component ownership rules in
`docs/ui-architecture.md` are binding. In particular:

- `Button`/`ActionButton`/`IconButton` own all action chrome and state;
- `ControlSurface` owns input, select and search chrome;
- `Surface` owns card and preview chrome;
- `PreviewSurface` owns History preview padding, scroll and focus;
- `MetadataList` owns semantic `dl/dt/dd` metadata;
- `StatusCard` owns status chrome, role, busy state and action layout;
- feature models own exhaustive domain presentation and remain React-free;
- feature screens own responsive geometry, not leaf-component styling.

Pass-through wrappers and component-valued escape-hatch props are not retained.
A shared primitive grows only finite modifiers required by more than one
consumer. PNG base64 decoding and object-URL lifetime have one shared helper;
replacement and unmount revoke each URL exactly once.

Every filling flex/grid child declares the minimum inline/block size needed to
shrink. One component owns each scroll container. Text truncates only when the
full value remains available through an adjacent accessible mechanism. Images
are contained, never cropped.

## 10. Responsive and platform rules

- One shared compact boundary selects dock versus sidebar.
- Library chooses resizable Inspector at the documented wide-screen boundary
  and detail dialog below it.
- Toolbar collapse belongs to its container-query owner; hiding optional count
  metadata precedes hiding actions.
- Settings tabs remain reachable at compact sizes and preserve semantic tab
  navigation or an equivalent compact ladder.
- Safe-area and coarse-pointer dimensions come from generated tokens.
- Platform actions use normalized capability/permission data.
- Device class uses generated values; display-name inference is forbidden.
- One viewport metrics provider owns resize observation. Visual responsiveness
  uses CSS media/container queries.

## 11. Acceptance tests

### 11.1 History and body safety

- Prepend, append, deletion and shrink preserve/clamp an item anchor.
- Equal idle polls keep list identity; a mutation invalidates it.
- Load-more remains merged after the next head refresh.
- Every card variant's reservation is at least its rendered cap and is replaced
  by measurement.
- Active row ids never point to unmounted elements and selection announcements
  update after keyboard movement.
- `truncated + full-body failure` renders unavailable in both Inspector and
  detail; success replaces the preview in both.
- Sensitive content remains absent from DOM, accessibility tree, search and
  caches until reveal, then re-hides on both timeout and visibility loss.
- Singular item labels and plural filter labels remain distinct; copy action
  presentation matches across surfaces.
- Unix/Windows filenames, origin fallback and absolute time use their canonical
  formatters.

### 11.2 Accessibility and layout

- Axe reports no nested-interactive, dialog, tab or required-child violations.
- Focus enters/traps/restores for dialogs and nested locks release in order.
- Icon-only controls have accessible names and pointer help.
- Alerts and statuses match the exhaustive presentation descriptor.
- Reduced-motion/transparency and every theme meet their gates.
- History, Settings, pairing and Capture remain operable at compact native
  widths and the wide Inspector layout.
- Metadata preserves valid `dl/dt/dd` structure and preview content wraps or
  scrolls without clipping.

### 11.3 Pairing and security

- QR payload text is absent from DOM snapshots, accessibility trees and logs.
- QR refresh pauses while hidden and re-protects a regenerated token.
- SAS digits are inert, metadata is labelled unverified and timeout removes
  Accept/Reject.
- Closing from every ceremony state sends cancel and clears UI state.
- Responder/initiator events cannot open the wrong modal and confirmation
  ownership is singular.
- Rendered error sweeps find no absolute POSIX, drive or UNC path, username,
  token or secret fixture.

### 11.4 Capture, permissions and settings

- Parameterized tests cover every capture health/next-step and permission enum
  value with one label, action, tone, disabled state and ARIA role.
- `not_required` is complete/non-actionable; denied and unavailable use their
  distinct recovery paths.
- Optimistic toggles revert only their field and busy state always releases.
- Failed restart removes success state; failed save prevents connection test.
- Blank credential fields preserve stored secrets and returned settings contain
  no credential value.
- Import/restore performs no mutation before confirmation.
- Per-field preference corruption preserves valid neighbors and first paint has
  no appearance flash.
- Shortcut capture is layout-independent and failure does not crash startup.

### 11.5 Popup, tray and lifecycle

- Quick Paste show/hide restores the correct application exactly once across
  blur/click races and copy failure keeps it open.
- Position clamps on primary and secondary monitors in every mode.
- Warm popup rereads preferences, clears item/image memory on hide and shows no
  false empty-history state while reloading.
- Number shortcuts disable during search; hover/scroll do not steal keyboard
  selection.
- Main-window close hides and Quit exits.
- Tray setup returns without IPC; background convergence updates private mode
  and recent items from daemon truth.
- Toasts stack, pause on hover/focus and do not cover dialogs.

### 11.6 Stable acceptance IDs

Source comments and focused tests cite these ids as compact links to the
current contract. Their meaning remains stable when test names or components
move.

| ID | Current v2 acceptance rule |
|---|---|
| **AT-8** | Every History row reservation covers the rendered cap at the narrowest supported width; measured rows never overlap. |
| **AT-10** | History arrows reveal off-screen rows without wrapping; Quick Paste arrows wrap. |
| **AT-24** | A raw error containing a filesystem path leaves no matching DOM text or accessible name. |
| **AT-29** | Pairing timeout removes SAS digits and decision controls and shows the terminal timeout state. |
| **AT-39** | Dismissing Quick Paste without a recorded target does not focus or promote the main window. |
| **AT-44** | Hiding Quick Paste clears its item list and image cache; showing it refetches the list. |
| **AT-49** | Persisted appearance is applied before the first painted application frame. |
| **AT-50** | One invalid preference falls back independently and does not discard valid neighboring preferences. |
| **AT-51** | Malformed, non-object or unreadable preference storage falls back safely without throwing. |
| **AT-52** | Unknown preference keys are ignored and are not written back. |
| **AT-53** | System appearance changes update every window live through one maintained media-query subscription. |
| **AT-54** | The pre-paint bootstrap and application preference schema agree on key, defaults, enums and normalized values. |
| **AT-56** | Shortcut capture uses the physical key code and is independent of keyboard layout. |
| **AT-57** | Bare modifiers and shortcuts with no modifier are rejected without changing the binding. |
| **AT-58** | Escape cancels shortcut capture without changing the binding. |
| **AT-73** | Search finds a matching row beyond the loaded History pages through the database-wide search command. |

## 12. Module and dependency rules

Use React Query for server state, TanStack Virtual for measured virtualization,
Radix primitives for focus-heavy controls and the shared observer/provider
infrastructure. Domain-to-presentation mappings are exhaustive records or
switches over generated types. A new hand-written enum, timer loop, focus trap,
object-URL lifecycle or media classifier requires evidence that no maintained
owner already exists.
