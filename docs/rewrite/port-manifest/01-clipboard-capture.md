# Port Manifest 01 — Clipboard Capture

This manifest specifies the current v2 clipboard-capture contract. Native
capture accepts one representation, plain text. A change that offers only an
image, file reference or rich text is acknowledged and skipped. Binary values
received through import or sync remain valid stored items and can be pasted
back; that does not make binary clipboard capture a product capability.

The implementation has one platform-neutral change tracker, one capture policy
and one ingest path. Platform backends own only the OS calls needed to observe
and read a value.

## 1. Responsibilities and platform posture

The capture boundary owns:

- deciding whether the clipboard changed before reading a representation;
- suppressing app-owned writes exactly once;
- applying platform opt-outs, private mode and source-app exclusions before a
  value enters process memory;
- reading and size-gating a plain-text value;
- classifying source-app sensitivity;
- handing the value to the shared encrypted ingest path;
- reporting lost intermediate changes and size rejections without exposing
  content.

macOS uses `NSPasteboard.changeCount`; Windows uses the system clipboard
sequence number. Both run the same change-tracker state machine and expose the
same counters. Linux is a test surface with a named fake backend, never a
shipping implementation. Android's user-mediated capture routes enter the same
product ingest policy but do not pretend that an unrestricted background
clipboard monitor exists.

Paste-back, content encryption, storage transactions, sync conflict resolution
and the secret-pattern ruleset are owned by their respective modules. Capture
may call those owners but may not restate their formats or decisions.

## 2. Change detection

### 2.1 Stable rules

- **I-1:** An unchanged sequence returns no content. The sequence comparison is
  the first operation; an idle poll performs no representation read.
- **I-2:** The initial cursor is outside the valid non-negative sequence domain.
  The first observation is a change, never a burst.
- **I-3:** Every drop path acknowledges the observed sequence. Opt-outs,
  self-writes, private mode, exclusions, empty text and unsupported formats must
  not be re-offered forever.
- **I-4:** Burst loss is computed from the cursor value that preceded the
  observation, then the cursor advances.

A sequence counter is lossy. When its delta proves that intermediate values
were overwritten, the surviving current value is still captured and the loss
counter increases. Burst telemetry is not a content variant and never replaces
the value that survived.

### 2.2 Self-write protocol

All app-owned clipboard writes share one sentinel with the capture source:

1. Read the current sequence.
2. Begin the native write and obtain the sequence generation that write owns.
3. Arm the sentinel with that observed generation before content becomes
   visible.
4. Commit the representation.
5. Clear the sentinel if the write fails.
6. A matching poll consumes the sentinel exactly once and acknowledges the
   change without reading it.

The write path must not predict a sequence delta. A non-matching observation is
another writer's value and must not be relabelled as the app's own write. Every
producer that writes a synced, menu, quick-paste or ordinary History value uses
the same primitive.

## 3. Privacy and representation selection

### 3.1 Pre-read privacy gates

- **I-5:** Platform do-not-record markers are probed before any representation
  is read. macOS checks all three `org.nspasteboard.*` opt-out types; Windows
  applies its maintained opt-out vocabulary.
- **I-6:** Private mode acknowledges changes and stores nothing.
- **I-7:** When source exclusions are configured and the source application
  cannot be attributed, capture fails closed for that change. With no exclusion
  configured, missing attribution alone does not suppress capture.
- **I-8:** Source attribution still runs when the exclusion set is empty because
  credential-store classification is an independent consumer.
- **I-9:** Logs and public errors contain no clipboard content, filename, path or
  recoverable content fingerprint. Bounded counts, sequence values, item ids and
  bundle/package identifiers are permitted.
- **I-10:** A plaintext dedup digest is never logged with correlating metadata.

Credential-store attribution is sufficient to mark an item sensitive even when
its text does not match a detector rule. Explicit user exclusion remains
stronger and prevents capture entirely.

### 3.2 Current representation contract

- **I-11:** If plain text is offered, it is the single captured value.
- A text read is bounded by the live text limit and the shared hard content cap.
  The smaller applicable bound wins.
- The native length is checked before copying bytes into an owned buffer.
- A non-text-only change is acknowledged without materialising image, file or
  rich-text bytes.
- Unsupported types may increment bounded telemetry, but their names and
  payloads are not logged repeatedly.

Binary paste-back has its own explicit API. A backend without that capability
refuses the operation instead of coercing bytes through text.

## 4. Resource and failure safety

- **I-17:** Every macOS poll and native write drains an autorelease pool around
  the complete Cocoa interaction.
- **I-18:** A platform length check precedes any potentially large allocation.
- **I-20:** SQLite, encryption, image work, filesystem reads and process work run
  off the async reactor. A database guard is never held across an await.
- **I-21:** Every helper process is reaped on success, failure and cancellation.
- **I-36:** A malformed value, platform error, blocking-task failure, detector
  failure, encryption failure or database failure cannot kill the monitor loop.
- **I-39:** A size rejection increments a readable diagnostic counter. It is not
  represented only by a log line.

An accepted capture retries only typed transient storage busy/locked and
interrupted/would-block/timed-out file failures. It retains its original payload
and timestamp, rechecks current privacy and exclusion policy before each retry,
and reads current retention at persistence. Policy cancellation, storage, and
permanent failure are distinct terminal outcomes; no capture event precedes
successful persistence.

The platform poll interval, live limits, private mode and exclusion policy are
read from current settings. A change takes effect without restarting the
daemon. The event channel may coalesce refresh work, but it must preserve
capture and auto-wipe counts that make data changes visible.

## 5. Ingest and identity

Capture passes through the same current ingest service as other local inserts.
It does not construct a storage row or encryption envelope independently.

- **I-22:** Encryption uses the current item key and item-id AAD selected by the
  read path. There is no key number, alternate AAD or trial-decrypt contract.
- **I-23:** Re-copying identical text converges to one logical item and refreshes
  its recency. Dedup searches the complete retained history, including pinned
  items.
- **I-28:** When ingest deduplicates against an existing row, downstream change
  notifications describe the stored winner, never the rejected candidate.
- **I-29:** A new capture receives a stable logical `item_id`, current timestamp
  and source-app metadata when known. Transport-specific ordering fields are
  derived by the sync owner, not stamped ad hoc by capture.
- **I-30:** Content detection and source-app classification are independent
  sensitivity signals; either is sufficient.
- **I-31:** A sensitive item's expiry uses the user-configured sensitive TTL.
- **I-32:** Re-copying a sensitive item recomputes expiry from the new capture
  time.
- **I-33:** A failed dedup lookup falls through to normal insert. A duplicate is
  safer than a lost capture.
- **I-34:** A row deleted concurrently between lookup and refresh produces no
  panic and no notification for a nonexistent row.
- **I-35:** Local persistence does not depend on any sync transport being
  enabled or reachable.

Sensitive items are excluded from search indexing at ingest and read back
through the sensitive-content contract. Capture never inserts into FTS
directly.

## 6. Source-app policy

The installed-application catalogue is the selection source for exclusions.
Persisted values are stable package/bundle identifiers; display names are
presentation only. Entries missing from the current launcher catalogue remain
removable so an uninstalled application cannot strand an exclusion forever.

Attribution work is cached for a short bounded interval, runs off the async
reactor and is invalidated on focus/application changes as the platform allows.
Resolution failure follows I-7 and never fabricates a source identity from a
display string.

## 7. Acceptance tests

### 7.1 Change tracker and self-writes

- An unchanged counter performs no representation access across repeated polls.
- The first observation at an arbitrary non-negative value reports no burst.
- A threshold-crossing delta captures the surviving value and reports only the
  number of overwritten intermediates.
- Privacy, unsupported-format and self-write drops advance the cursor and are
  not re-offered.
- A successful app write is suppressed once. A failed write clears the
  sentinel. A genuine write beside an armed, non-matching sentinel is captured.
- Every app-owned write route uses the same sentinel instance.

### 7.2 Privacy and limits

- Each platform opt-out marker independently prevents a content read; mixed
  markers do the same.
- Private mode stores nothing and disabling it does not replay values copied
  while it was active.
- Unknown attribution with a non-empty exclusion list skips; the same unknown
  attribution with an empty list captures.
- A known credential-store origin marks otherwise unremarkable text sensitive.
- The exact size boundary succeeds, one byte over fails before owned allocation,
  and the readable rejection counter increases.
- Captured content, paths and fingerprints are absent from logs and rendered
  errors on success and failure paths.

### 7.3 Representation and ingest

- A mixed clipboard offering text plus any binary format captures text without
  reading the binary representation.
- A non-text-only change is acknowledged and creates no row.
- Empty, malformed and invalid-UTF-8 platform values fail without panic or
  monitor termination.
- Identical text creates one row and refreshes it; a dedup-query failure still
  preserves the new capture.
- Dedup notification ids always resolve to a stored row.
- Sensitive detection or credential-store attribution keeps the item out of
  search, and re-copy refreshes its sensitive expiry.
- Disabled, offline or failing sync never prevents local storage.

### 7.4 Platform and lifecycle

- macOS and Windows backends run the shared change-tracker suite.
- Native platform tests prove the unchanged fast path, opt-out probes,
  self-write suppression, size boundary and source attribution against the real
  clipboard API.
- The fake backend identifies itself in status and cannot be mistaken for a
  shipping backend.
- A capture storm cannot kill the poll loop or overflow into plaintext-bearing
  events.
- Long-running blocking work does not stall service status, shutdown or another
  ready request.
- A shutdown keeps its endpoint and refuses new mutations until an accepted
  capture and every admitted request reach an explicit terminal outcome.
- A transiently busy accepted capture can drain past the cooperative shutdown
  budget without polling a newer value; permanent persistence failure or a
  blocking-task panic makes shutdown fail after ownership cleanup.

## 8. Module and dependency rules

`ClipboardSource` is the platform seam. The pure change tracker owns sequence
and self-write state exactly once. Platform modules translate native values and
apply pre-read gates; the capture service owns orchestration; core ingest owns
dedup, sensitivity, encryption and persistence.

Use maintained platform bindings, property-list/URL parsers, hashing,
content-type and async blocking facilities. A platform backend may not add a
second tracker, detector, row constructor or format parser hidden behind its
native module.
