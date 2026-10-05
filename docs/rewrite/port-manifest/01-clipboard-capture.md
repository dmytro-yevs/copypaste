# Port Manifest 01 — Clipboard Capture

This manifest specifies the current v2 clipboard-capture contract. Native
capture selects one representation: a native single File first, then plain text,
RTF, HTML, PNG, TIFF (plus the existing Windows bitmap fallback). A native File
wins over a textual filename or path; a failed selected File never falls back
to Text. Without a native File, plain text wins over the remaining fallbacks.
File capture freezes bytes of one accessible nonempty regular local file on an
admitted blocking worker, within the live file limit and shared hard cap. It
retains the resolved absolute locator and existing FileMetadata. Zero-byte
files and multiple total pasteboard items remain explicitly unsupported.

The implementation has one platform-neutral change tracker, one capture policy
and one ingest path. Platform backends own only the OS calls needed to observe
and read a value.

## 1. Responsibilities and platform posture

The capture boundary owns:

- deciding whether the clipboard changed before reading a representation;
- suppressing app-owned writes exactly once;
- applying private mode and source-app exclusions before a representation is
  accessed under the platform's available source evidence;
- selecting, materializing and size-gating one text, image or supported File value;
- handing the value to the shared encrypted ingest path;
- reporting lost intermediate changes and size rejections without exposing
  content.

macOS uses `NSPasteboard.changeCount`; Windows uses the system clipboard
sequence number. Both run the same change-tracker state machine and expose the
same counters. Linux is a test surface with a named fake backend, never a
shipping implementation. Android's user-mediated capture routes enter the same
product ingest policy but do not pretend that an unrestricted background
clipboard monitor exists.

Paste-back, content encryption, storage transactions and sync conflict
resolution are owned by their respective modules. Capture may call those owners
but may not restate their formats or decisions.

## 2. Change detection

### 2.1 Stable rules

- **I-1:** An unchanged sequence returns no content. The sequence comparison is
  the first operation; an idle poll performs no representation read.
- **I-2:** The initial cursor is outside the valid non-negative sequence domain.
  The first observation is a change, never a burst.
- **I-3:** Every drop path acknowledges the observed sequence. Self-writes,
  private mode, exclusions, empty text and unsupported formats must not be
  re-offered forever.
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

- **I-5:** Third-party sensitivity and opt-out metadata do not independently suppress capture. No sensitivity detector or marker filter is enabled.
- **I-6:** Private mode acknowledges changes and stores nothing.
- **I-7:** With exclusions configured, macOS denies incomplete generation
  coverage or a candidate set containing any excluded app before type, data,
  native object or file access. Complete known-allowed coverage may capture
  with ambiguous display identity. Generic missing owner evidence fails closed;
  an empty exclusion list permits unavailable identity.
- **I-8:** macOS source metadata is a foreground-derived estimate. Only complete
  single-candidate evidence retains a name/bundle; ambiguous or unavailable
  evidence leaves both fields absent. Windows preserves identified clipboard
  owner metadata. Android denies implicit capture with configured exclusions
  because it has no source witness. The shared inspector says Observed app;
  absent metadata omits the identity component rather than inventing an app.
- **I-9:** Logs and public errors contain no clipboard content, filename, path,
  URL or recoverable content fingerprint. File capture emits only fixed stage
  categories and bounded events, never item ids, digests, source identities,
  native error descriptions or dynamic I/O errors.
- **I-10:** A plaintext dedup digest is never logged with correlating metadata.

macOS skips observed excluded activity. Arbitrary unobserved background or
delayed writers may bypass exclusions; foreground/count evidence does not
authenticate the clipboard writer. Pause remains the way to stop all automatic
capture. Recorded fences reject an invalidated read result before ingest or
publication, but cannot undo an OS read already raced by an unobserved writer.

### 3.2 Current representation contract

- **I-11:** Select native File before text, RTF, HTML, PNG and TIFF. Without
  File, offered plain text is the single captured value. A selected
  representation's failure is terminal for that acknowledged generation.
- Text, image and File values use min(live content limit, shared hard cap).
- Native data length is checked before copying representation bytes. For macOS
  File, require exactly one total pasteboard item before the length preflight;
  do not copy or parse the raw URL data. Read only NSURL objects with the
  file-only option, require one NSURL and resolve with filePathURL.
- macOS accepts only local file URLs with empty/localhost authority and no
  credentials, port, query or fragment. A bounded filesystem representation
  becomes an absolute PathBuf with strict UTF-8 locator/basename metadata.
  File-reference URLs resolve natively; paths are not percent-decoded again or
  rewritten with guessed synthetic-reference rules.
- Keep the original NSURL alive through the synchronous read. Temporary
  security-scope access stops exactly once only when start succeeded. A false
  start still permits ordinary filesystem access; scope does not bypass TCC.
- **I-16:** All file I/O runs on the admitted blocking worker. macOS returns
  owned FILE bytes plus existing metadata and no deferred path after its final
  generation/coverage fence. Windows keeps its native deferred path output;
  capture normalizes it once under policy authority before Pending acceptance.
- Open one read-only descriptor, then check descriptor regularity and nonzero
  length before allocation. Unix uses CLOEXEC, NONBLOCK and NOCTTY; this avoids
  FIFO-open waits and controlling-terminal acquisition, not all possible I/O
  delays. Preserve symlinks to regular files. Read into at most observed length
  plus one sentinel byte; never use an unbounded growing read_to_end allocation.
  Reject partial reads or observed length/modification/descriptor changes.
- Zero-byte, multi-item, directory/nonregular, nonlocal, missing/unreadable,
  unavailable native object/resolution and invalid metadata inputs are typed
  terminal rejections. They create no row or Text fallback. No file promise,
  bookmark, coordination or provider download workflow is implemented.
- A normal accessible resident provider file may follow the local route. OS
  reads may implicitly trigger provider work or block indefinitely. No atomic
  filesystem snapshot or hard read-duration bound is promised; consistency
  checks can miss concurrent same-size writes with coarse timestamps.
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
- **I-36:** A malformed value, platform error, blocking-task failure,
  encryption failure or database failure cannot kill the monitor loop.
- **I-39:** Native adapter size rejections increment the existing readable
  adapter counter. macOS includes its admitted File descriptor-cap rejections.
  Shared deferred-file normalization and later live-policy or ingest-size
  rejections are outside that counter and use the bounded outcome/event path.
  The adapter counter is not a total of every rejected input.

An accepted capture retries only the existing typed transient storage failures:
busy/locked databases and interrupted/would-block/timed-out storage file I/O.
Input-file failures are distinct from policy cancellation, core Empty and
storage failure; they are never storage retries. Before Pending acceptance,
deferred file input is read once into owned bytes. Every retry retains exactly
those bytes, FileMetadata, source evidence and original timestamp; changing or
deleting the source cannot change the retry payload.

Desktop settings changes and one complete capture read/ingest/announcement
attempt share Settings.applying authority. The current-settings RwLock is held
only for the short snapshot, never across native/file/storage I/O. If settings
wins first, denied later attempts make no payload/file calls. If capture wins,
its admitted operation may finish before the settings response. Each retry
reacquires authority and checks live limits and the captured privacy epoch.
Private-mode or exclusion transitions revoke Pending even across A to B to A;
a later clipboard generation alone does not revoke an already frozen payload.
No announcement occurs until shared core persistence succeeds. Read or storage
calls may delay the operation and settings response; cancellation is cooperative.

The platform poll interval, live limits, private mode and exclusion policy are
read from current settings. A change takes effect without restarting the
daemon. The event channel may coalesce refresh work, but it must preserve
capture counts that make data changes visible.

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
- **I-33:** A failed dedup lookup falls through to normal insert. A duplicate is
  safer than a lost capture.
- **I-34:** A row deleted concurrently between lookup and refresh produces no
  panic and no notification for a nonexistent row.
- **I-35:** Local persistence does not depend on any sync transport being
  enabled or reachable.

Capture never inserts into FTS directly.

File bytes use the existing core binary identity, AEAD envelope, dedup and
SQLCipher persistence. FileMetadata remains in its existing metadata column;
this repair changes no wire, envelope, database or sync schema. Metadata retains
basename, generic MIME and optional original source_reference, and existing
authenticated sync may carry the locator. It is not an ongoing access token or
promise that the source remains unchanged or available.

Locator-bearing File Copy writes the stored Path/URI as text through the shared
write_payload/self-write route, even after source deletion. Reference-free
legacy File Copy retains existing authenticated-byte staging and native
paste-back. No source rename, rewrite, deletion, permission broadening, bookmark
store or plaintext metadata cache is part of capture. source_available remains
the existing local-origin/is_file heuristic, not proof of access or byte freshness.

## 6. Source-app policy

The installed-application catalogue is the selection source for exclusions.
Persisted values are stable package/bundle identifiers; display names are
presentation only. Entries missing from the current launcher catalogue remain
removable so an uninstalled application cannot strand an exclusion forever.

macOS source observation is owned on the original main thread, while typed
generation coverage and admission run on the blocking poll worker. Coverage
retains excluded/unknown debt across unchanged samples. Every fresh accepted or
rejected generation consumes only its sampled interval and retains later real
events. A consumed Unknown boundary can recover through a same-count,
service/event-fenced known getter when no later real event or gap is skipped;
idle duration alone cannot erase debt. A clean later interval must recover.

Only a complete single-candidate interval exposes an observed name/bundle.
Multiple known allowed candidates may permit capture with null identity.
Incomplete/unknown/service-reset/evicted coverage denies with exclusions. Public
foreground/count observations cannot identify arbitrary unobserved background
writers or prove that several delayed writes belonged to the consumed interval.
The Settings wording reflects these platform limits; old/synced metadata is not
migrated or guessed. Windows retains its owner resolver; Android admission is
unchanged. The shared SourceAppLabel owns every displayed icon/name pair.

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

- Sensitivity or opt-out metadata does not add a capture gate. Private mode, explicit exclusions, generation fences and size limits remain independently enforced.
- Private mode stores nothing and disabling it does not replay values copied
  while it was active.
- Incomplete coverage with exclusions skips without type/data/object/file calls;
  empty exclusions permit unknown source. Complete all-known-allowed ambiguous
  coverage captures with absent identity. Any excluded candidate denies.
- Unchanged polls preserve excluded/unknown debt; consumed Unknown recovery
  permits the first clean later generation without erasing later real events.
- Generation/service/coverage replacement during decode, resolution or file
  read prevents publication; newer generations remain available.
- Both config routes serialize capture and response. Settings first makes no
  reads; capture first finishes announcement before settings response. Pending
  private/exclusion A to B to A cancels without read, ingest or announcement.
- Exact native representation size boundaries succeed; one byte over is
  rejected before copying into an owned buffer and increments the adapter
  counter. macOS File descriptor-cap rejections also increment that counter.
  Deferred-file descriptor and later live-policy/ingest cap rejections assert
  their bounded outcomes separately, without an adapter-counter increase.
- Captured content, paths and fingerprints are absent from logs and rendered
  errors on success and failure paths.

### 7.3 Representation and ingest

- A single native File plus textual filename/path captures File once. Without
  File, plain text wins over rich-text/image representations. A failed selected
  File creates no Text fallback.
- A supported non-text fallback captures one RTF, HTML, image, or local file
  representation; unsupported and multi-file changes are acknowledged without
  creating a row.
- Empty text is skipped; malformed text preserves the established lossy UTF-8
  behavior. Invalid File URL/path/metadata is explicitly rejected without lossily
  inventing its locator. Zero-byte and multi-item File inputs are unsupported.
- NSURL path and asserted file-reference fixtures preserve 32-byte no-extension,
  87-byte Unicode/spaces and literal percent/hash names. Native fixtures are
  isolated and require separate execution authorization.
- Descriptor regularity, exact-cap success, oversize, missing/denied/nonregular,
  partial-read/mutation, bounded allocation and scope balancing are covered.
- Busy retries preserve File bytes/evidence/time after source mutation/deletion,
  with no event before persistence. Locator text Copy and reference-free legacy
  staging remain compatible; shared encrypted-byte/metadata roundtrip succeeds.
- Identical captured content creates one row and refreshes it; a dedup-query
  failure still preserves the new capture.
- Dedup notification ids always resolve to a stored row.
- Disabled, offline or failing sync never prevents local storage.

### 7.4 Platform and lifecycle

- macOS and Windows backends run the shared change-tracker suite.
- Native platform tests prove the unchanged fast path, self-write suppression,
  size boundary and source attribution against the real clipboard API.
- The fake backend identifies itself in status and cannot be mistaken for a
  shipping backend.
- A capture storm cannot kill the poll loop or overflow into plaintext-bearing
  events.
- Blocking work runs off the reactor. An admitted in-flight native/file/storage
  operation may delay serialized settings or cooperative shutdown; no arbitrary
  syscall cancellation or prompt-response bound is claimed.
- A shutdown keeps its endpoint and refuses new mutations until an accepted
  capture and every admitted request reach an explicit terminal outcome.
- A transiently busy accepted capture can drain past the cooperative shutdown
  budget without polling a newer value; permanent persistence failure or a
  blocking-task panic makes shutdown fail after ownership cleanup.

## 8. Module and dependency rules

`ClipboardSource` is the platform seam. The pure change tracker owns sequence
and self-write state exactly once. Platform modules translate native values and
apply pre-read gates; the capture service owns orchestration; core ingest owns
dedup, encryption and persistence.

Use maintained platform bindings, property-list/URL parsers, hashing,
content-type and async blocking facilities. A platform backend may not add a
second tracker, row constructor or format parser hidden behind its
native module.
