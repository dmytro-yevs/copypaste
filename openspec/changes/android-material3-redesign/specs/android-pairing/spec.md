## ADDED Requirements

### Requirement: QR pairing display and lifecycle

The pairing screen SHALL display a QR code for pairing with a visible lifetime countdown/progress
indicator, a near-expiry warning, and a regenerate action once expired or on demand. The QR code
SHALL render blurred at rest and require explicit user action to reveal, consistent with STYLEGUIDE
§7 masked-content handling.

#### Scenario: QR blurs at rest and reveals on demand
- **WHEN** the QR pairing screen first appears
- **THEN** the QR code renders blurred and only becomes legible after the user performs the explicit
  reveal action

#### Scenario: Lifetime countdown reaches the warning threshold
- **WHEN** the QR code's remaining lifetime crosses the near-expiry threshold
- **THEN** a warning is shown alongside the progress indicator, before the code actually expires

#### Scenario: Expired QR offers regenerate
- **WHEN** the QR code's lifetime elapses
- **THEN** the code is marked expired and a regenerate action is offered in its place

#### Scenario: Regenerating issues a fresh code
- **WHEN** the user taps regenerate (on expiry or on demand)
- **THEN** a new QR code is displayed and its lifetime/progress indicator resets from full

### Requirement: Scan initiation and scan-review

The app SHALL support launching an in-app camera scan flow and accepting `cppair://` deep links as
equivalent entry points into pairing. After a successful scan or deep-link resolution, the app SHALL
present a scan-review card summarizing the discovered peer before proceeding to verification.

#### Scenario: In-app scan launch opens the camera flow
- **WHEN** the user chooses to scan a peer's QR code from within the app
- **THEN** the camera scan flow launches and, on a successful decode, proceeds to the scan-review card

#### Scenario: Deep link launches pairing directly to scan-review
- **WHEN** the app is opened via a `cppair://` deep link
- **THEN** pairing starts and the scan-review card is shown with the peer data resolved from the link,
  without requiring a manual camera scan

#### Scenario: Scan-review requires explicit confirmation
- **WHEN** the scan-review card is shown for a discovered peer
- **THEN** the user must explicitly confirm before the flow proceeds to SAS verification

### Requirement: SAS confirmation is the six-digit code (fingerprint is supplemental)

The Short Authentication String (SAS) confirmation step SHALL present the **six-digit SAS code** as
the primary match decision, with Match / Doesn't-match actions, and SHALL preserve the existing
polling (`SAS_POLL_MS`), watchdog/timeout (`SAS_WATCHDOG_MS`), waiting, and terminal states. The full
64-hex fingerprint MAY be shown as supplemental metadata but SHALL NOT replace or obscure the SAS
code. Neither the SAS code nor any pairing token SHALL be written to logs or notifications.

#### Scenario: Six-digit SAS is primary
- **WHEN** the SAS state is `awaiting_sas` with a code
- **THEN** the six-digit SAS is displayed with Match / Doesn't-match actions; the fingerprint, if
  shown, is clearly supplemental and does not replace the SAS

#### Scenario: Waiting and watchdog states preserved
- **WHEN** `awaiting_sas` has no code yet, or the watchdog elapses
- **THEN** a waiting state ("Waiting for the other device…") or a timeout/abort terminal state is
  shown, matching current behaviour

#### Scenario: User confirms or rejects the match
- **WHEN** the user taps Match or Doesn't-match
- **THEN** the flow proceeds to bootstrap on Match, or cancels on Doesn't-match

#### Scenario: SAS never leaks to logs
- **WHEN** the SAS flow runs
- **THEN** neither the SAS code nor session/pairing tokens appear in logcat or notifications

### Requirement: Pairing progress and success states

The pairing flow SHALL surface distinct connecting, provisioning, bootstrap, and sync-progress states
while the handshake and initial data exchange proceed, and SHALL present a success popup on
completion.

#### Scenario: Each progress phase is distinguishable
- **WHEN** pairing advances from connecting through provisioning, bootstrap, and sync
- **THEN** the screen shows a distinct label/indicator for the current phase at each step

#### Scenario: Success popup may show a truncated fingerprint
- **WHEN** pairing completes successfully
- **THEN** the success popup shows the paired peer's fingerprint truncated as `first16…last8`, which
  is acceptable at this post-verification step

### Requirement: Pairing error and recovery states

The pairing flow SHALL handle and present recoverable error states: invalid QR code, expired QR code,
camera-permission-denied, and network/protocol errors, each offering a cancel action and, where
applicable, a retry action.

#### Scenario: Invalid QR code is rejected with a clear message
- **WHEN** a scanned or deep-linked code fails to parse as a valid pairing QR
- **THEN** an invalid-QR error state is shown with cancel and retry actions

#### Scenario: Expired QR code is distinguished from invalid
- **WHEN** a scanned code parses but its embedded lifetime has elapsed
- **THEN** an expired-QR error state is shown, distinct from the invalid-QR state, with retry offered

#### Scenario: Camera permission denial is recoverable
- **WHEN** the user denies camera permission during scan launch
- **THEN** a camera-denied state is shown with guidance to grant permission, and a cancel action

#### Scenario: Network or protocol error allows cancel or retry
- **WHEN** the pairing handshake fails due to a network or protocol error
- **THEN** an error state names the failure and offers cancel and retry actions

### Requirement: Preserved unconditional FLAG_SECURE during pairing

`PairActivity` SHALL continue to set `FLAG_SECURE` unconditionally for the entire pairing flow,
independent of the `allowScreenshots`/translucency preference, because PAKE and provisioning secrets
are on screen during pairing.

#### Scenario: FLAG_SECURE is set regardless of the screenshot preference
- **WHEN** `PairActivity` is composed, whether `allowScreenshots` is true or false
- **THEN** `FLAG_SECURE` is set on the activity's window in both cases

### Requirement: Preserved pairing IPC and account-linkage semantics

The redesign SHALL NOT alter the `PairController` / `PairProvisioning` / `PairBootstrapSync` IPC
contracts, SHALL continue sending `peer_supabase_account_id=None`, and SHALL leave revoke semantics
(local-only, no peer notification) unchanged.

#### Scenario: IPC call shapes are unchanged after the redesign
- **WHEN** the redesigned pairing screens invoke `PairController`/`PairProvisioning`/`PairBootstrapSync`
- **THEN** the calls use the same method signatures and payloads as before the redesign

#### Scenario: peer_supabase_account_id remains unplumbed
- **WHEN** a pairing handshake completes
- **THEN** the peer record is created with `peer_supabase_account_id=None`, unchanged from current
  behaviour

### Requirement: Scanner window must set FLAG_SECURE (SECURITY)

`PortraitCaptureActivity` SHALL set `FLAG_SECURE` on its window before the camera preview renders,
because the scanner preview necessarily shows the **peer's pairing QR**, which encodes pairing
material (fingerprint + token); a screenshot or recents capture of the scanner would capture a still-
valid pairing credential. This reverses the earlier (incorrect) "no FLAG_SECURE accepted" decision.
The redesign SHALL otherwise own only the scanner's theme, portrait orientation lock, and decoder
configuration, leaving ZXing's internal preview UI unskinned.

#### Scenario: Scanner blocks screenshots/recents
- **WHEN** `PortraitCaptureActivity` is launched
- **THEN** `FLAG_SECURE` is set before the preview renders, so screenshots and the recents thumbnail
  are blocked for the scanner window

#### Scenario: Only theme/orientation/decoder are app-controlled
- **WHEN** the scanner runs
- **THEN** only its theme, portrait lock, and decoder are app-controlled; ZXing's preview UI is otherwise untouched

#### Scenario: Window-flag coverage is tested
- **WHEN** the connected window-flag test suite runs
- **THEN** it asserts `FLAG_SECURE` is set on both `PairActivity` and `PortraitCaptureActivity`

#### Scenario: External scanner apps are out of scope
- **WHEN** the user pairs via an external camera/scanner app instead of the in-app scanner
- **THEN** that app is outside CopyPaste's control and this requirement does not apply to it (documented limitation)
