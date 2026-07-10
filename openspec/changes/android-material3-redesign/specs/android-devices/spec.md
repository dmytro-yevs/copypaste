## ADDED Requirements

### Requirement: Own-device card field grid

The own-device card SHALL present a Model / OS / Version / Local IP / Public IP / Fingerprint field
grid sourced from `OwnDeviceInfo`, laid out as a natural-height, baseline-aligned grid with dim 11px
labels and faint 11px mono tabular-nums values (STYLEGUIDE §9.7). It SHALL NOT render any footer
action (Unpair/Revoke), since the row represents the current device.

#### Scenario: Own device renders six fields
- **WHEN** the own-device card is composed
- **THEN** it shows exactly Model, OS, Version, Local IP, Public IP, and Fingerprint as label/value
  pairs, with no Unpair or Revoke action visible

#### Scenario: Own device grid is natural height
- **WHEN** the own-device card (6 rows) is laid out next to a taller paired-peer card (8 rows)
- **THEN** the own-device card does not stretch to match the taller card's height and shows no
  internal gap

### Requirement: Paired-peer card field grid

The paired-peer card SHALL present an 8-field grid — Model, OS, Version, Local IP, Public IP, Paired,
Last sync, RTT — sourced from `PairedDevice`, using the same grid mechanics as the own-device card
(dim 11px labels, faint 11px mono tabular-nums values, `word-break: break-all`). A field row SHALL be
hidden only when its value is genuinely absent.

#### Scenario: Fully-synced peer shows all eight fields
- **WHEN** a paired peer has an active P2P link and a recent sync
- **THEN** the card renders all eight fields with real values, so it reads with the same weight as
  every other peer

#### Scenario: RTT shows a placeholder without a live P2P link
- **WHEN** a paired peer has no live P2P connection
- **THEN** the RTT field is still rendered, displaying "—" rather than being hidden

#### Scenario: Paired date is absolute, last sync is relative-then-absolute
- **WHEN** the card renders the Paired and Last sync fields
- **THEN** Paired shows an absolute date from `added_at`, and Last sync shows a relative time from
  `last_sync_at` when within 24 hours and an absolute date beyond that

### Requirement: Fingerprint tap-to-copy parity

Every device card — own device, every paired-peer card, and the pairing roster — SHALL render its
fingerprint truncated as `first16…last8` of the 64-hex value and SHALL make it tap-to-copy, copying
the full 64-hex fingerprint to the clipboard on tap. This is NEW behaviour for all three surfaces —
today neither the own-device card (`OwnDeviceRow` renders its fingerprint via the non-interactive
`MetaRow` in `DevicesUtils.kt`) nor the paired-peer card (`PeerRow.kt`) has tap-to-copy on the Devices
screen. The pattern to reuse is `PairedPeerList.kt`'s `onCopyFingerprint` callback from the Pairing
flow (S8), adapted here for the Devices screen (S7).

#### Scenario: Tapping a peer's roster fingerprint copies the full value
- **WHEN** the user taps the truncated fingerprint on a paired-peer roster card
- **THEN** the full 64-hex fingerprint is copied to the clipboard — this is new behaviour on the
  Devices screen, adapted from `PairedPeerList.kt`'s `onCopyFingerprint` pattern used during Pairing

#### Scenario: Truncated display never varies by surface
- **WHEN** the same device's fingerprint is shown on the own-device card, the paired-peer card, and
  the roster
- **THEN** all three render the identical `first16…last8` truncation

### Requirement: Status badges rendered as pills and chips

Transport (P2P/Cloud), This-device, and Verified indicators SHALL render as the pill/chip components
defined in STYLEGUIDE §9.4 — filled `--r-pill`/`--r-chip` shapes with tinted background and colored
text or dot — rather than plain, unstyled text. This corrects the current implementation, where these
indicators render as plain `Text`.

#### Scenario: Transport pill reflects connection type
- **WHEN** a paired peer's active transport is P2P
- **THEN** its header shows a pill-shaped transport chip tinted with the P2P color
- **AND** a Cloud-transport peer shows the Cloud-tinted pill instead, never plain text

#### Scenario: This-device pill marks the own-device header
- **WHEN** the own-device card is composed
- **THEN** its header shows a pill-shaped "This Mac" / "This phone" chip using the accent-2-on-accent
  tint, not plain text

#### Scenario: Verified badge accompanies every paired peer
- **WHEN** a paired-peer card is composed
- **THEN** a hairline-bordered Verified badge with a status dot renders below the header, before the
  field grid, not as plain text

### Requirement: Device presence and lifecycle states

The Devices screen SHALL support and visually distinguish the following states: own device, paired
online, paired offline, discovered (unpaired, nearby), scanning, no-peers empty, reconnecting, and
error. Presence SHALL be encoded as both a status-dot color and a text label — color is never the sole
signal (STYLEGUIDE §7).

#### Scenario: Online vs. offline peer is distinguishable without color
- **WHEN** a paired peer's connection drops from online to offline
- **THEN** the status dot changes color **AND** the adjacent text label changes from "online" to
  "offline"

#### Scenario: Empty state when no peers are paired
- **WHEN** the device list contains only the own device and no paired peers
- **THEN** a centered empty state (icon + headline + hint, STYLEGUIDE §9.10) renders, distinct from
  the scanning state

#### Scenario: Reduced motion disables the presence glow
- **WHEN** the system reduced-motion setting is enabled
- **THEN** the online-presence dot renders as a static colored dot with no glow animation

### Requirement: Equal-width danger footer actions

Each paired-peer card SHALL render a footer, separated from the field grid by a top hairline,
containing full-width Unpair and Revoke actions of equal width, both styled as `danger` buttons
(STYLEGUIDE §9.1).

#### Scenario: Unpair and Revoke share the footer equally
- **WHEN** a paired-peer card's footer is composed
- **THEN** Unpair and Revoke each occupy 50% of the footer width and both use the danger button style
  (`err @ 9%` fill, `--err` text, `err @ 40%` border)

### Requirement: Unpair and revoke confirmation dialogs

The Devices screen SHALL present modal confirmation dialogs (STYLEGUIDE §9.9) for: plain unpair,
revoke-only, revoke-with-key-rotation (including a passphrase field with in-flight and
invalid-passphrase states), a revoke-error state, and a revoke-all confirmation with an in-flight
state.

#### Scenario: Unpair requires confirmation naming the device
- **WHEN** the user taps Unpair on a peer card
- **THEN** a modal confirming the action names the specific device and offers ghost Cancel / danger
  Unpair actions

#### Scenario: Revoke-with-rotation validates the passphrase in-flight
- **WHEN** the user submits a passphrase on the revoke-and-rotate dialog
- **THEN** the dialog shows an in-flight (loading) state while validating
- **AND** shows an invalid-passphrase error state if validation fails, without dismissing the dialog

#### Scenario: Revoke error is surfaced, not silently swallowed
- **WHEN** a revoke operation fails
- **THEN** the dialog shows a revoke-error state with the failure reason, and the user can retry or
  cancel

#### Scenario: Revoke-all requires a distinct confirmation
- **WHEN** the user triggers revoke-all
- **THEN** a separate confirmation dialog names the action as irreversible for all peers
- **AND** shows an in-flight state while the batch operation runs

### Requirement: Preserved revoke ordering, local-only semantics, and inert cloud-account-mismatch detection

The redesign SHALL preserve existing security-critical invariants: revoke operations SHALL write the
audit-log entry before removing the device from the roster (never reordered); Unpair and Revoke SHALL
remain local-only operations that send no signal to the peer device, and the UI SHALL keep the
existing warning copy stating this limitation; and `detectCloudAccountMismatch` SHALL remain inert —
Android continues to send `peer_supabase_account_id=None` and feeds the mismatch detector an empty
list, and the redesign SHALL NOT wire up or activate the cloud-account-mismatch banner
(CopyPaste-gldr).

#### Scenario: Audit log is written before roster removal
- **WHEN** a revoke operation completes
- **THEN** the audit-log write is observed to occur before the device is removed from the paired-device
  roster

#### Scenario: Unpair/Revoke warning copy remains visible
- **WHEN** the user opens the Unpair or Revoke dialog
- **THEN** the dialog states that the action is local-only and does not notify the peer device

#### Scenario: Cloud-account-mismatch banner never appears
- **WHEN** the redesigned Devices screen is rendered with any set of paired peers
- **THEN** no cloud-account-mismatch banner is shown, because `peer_supabase_account_id` is `None` and
  the mismatch detector receives an empty list

### Requirement: Device presentation reuses the existing roster/identity models

The redesign SHALL present devices by reading the existing `PairedPeer` roster model
(`PeerRoster.kt`) and `P2pIdentity` (own-device identity) DIRECTLY, and SHALL NOT introduce a new
`OwnDeviceInfo`/`PairedDevice` presentation-DTO layer. Fix round (S6-S8 review): this requirement
originally proposed those two names as NEW presentation DTOs that S7 would introduce; S7 instead
built the STYLEGUIDE §9.7 grid straight from `PairedPeer`/`P2pIdentity` with no adapter/DTO layer in
between (reuse-first: composables such as `DevicesRevokeActions`/`DevicesUtils`/`PeerRow` take
`PairedPeer` as their parameter type) — this requirement is retained to record that decision, not
to describe types that exist.

#### Scenario: Devices screen renders directly from PairedPeer/P2pIdentity
- **WHEN** the Devices screen is implemented
- **THEN** the paired-device grid and own-device card are built in S7 directly from `PairedPeer`
  and `P2pIdentity`, mapping their existing fields onto the STYLEGUIDE §9.7 grid
- **AND** no new `OwnDeviceInfo`/`PairedDevice` presentation DTO is introduced
