# File offers and explicit downloads

Files selected through Add file are registered immediately as deferred original
references. Registration reads metadata, not the file body. Their encrypted offer
contains a unique source version, size, name, MIME type, reference and origin.
The original must remain accessible; removing or changing it can make transfer
fail. Android retains the document read grant and opens its descriptor on demand.
Clipboard capture and previously imported files retain their encrypted snapshots.
Ordinary P2P and cloud History synchronization carries only the authenticated
offer, never the file body. The offer is the row's encrypted payload and is small enough for
the existing History transport budget. Text and copied images retain automatic
synchronization; receiving a file never changes the system clipboard.

The destination's Download action opens a separate paired Noise session to the
origin device. A direct authenticated route to that paired device is required;
cloud History stores offers and does not act as a file-body relay.
File requests contain only an existing History item ID, digest
and chunk index, never a filesystem path. The sender resolves the authenticated
local offer and either its encrypted snapshot or its own registered source,
checks current sharing policy and services one ordered 64 KiB chunk at a time.
A deferred source has no precomputed digest: the sender hashes while streaming
and sends an authenticated checksum trailer after validating the source. The
receiver requires that trailer and verifies its accumulated digest before
committing. The source version prevents equal-sized files sharing an identity. Changing item identity, digest or order is
refused. Unpairing or revoking a device cancels its active transfer lease.

The receiver checks the live local file limit before connecting, enforces exact
chunk sizes and order, and verifies the total size and SHA-256 before reporting
success. Cancellation, loss of the connection, integrity failure and write
failure remove the partial destination. Explicit downloads are serialized per
host; node-level file sessions are also bounded. Shutdown cancels active work.
An unavailable source produces a recorded failure. No byte download is
scheduled without an explicit action. Existing paired-device reachability behavior is unchanged.
Returning from the Android file picker suppresses a foreground reread only when
the platform clipboard generation is unchanged. Missing generation evidence
(including Android versions before API 26) preserves normal capture, and a new
copy made while the picker is open is still captured.

File bodies are encrypted in independently authenticated 64 KiB database chunks
bound to their digest and index. Identical snapshots share these encrypted
chunks. Row deletion, tombstoning and retention release the final reference and
its chunks in the same transaction. Backups and restores include chunks and
links. Snapshot capture/import is two-pass (hash, then encrypt) and validates the second
pass against the first before committing. Add file uses the deferred source
contract above and publishes its row and local reference in one transaction. A snapshot currently holds one SQLite
write transaction; large imports can delay other writers. Imports do not retain
a file-sized plaintext or ciphertext buffer.

Already-retained inline files are converted before production networking starts.
Their item IDs, origins, names, pin state and bytes survive; the offer has a new
version stamp so peer and cloud cursors observe the representation change.
Unreadable legacy rows remain intact and counted by existing unreadable-row
handling. Legacy decryption uses the previous 4 MiB ceiling.

Download workers belong to the shared runtime/daemon, not the initiating UI
engine or its request connection. Closing Quick Paste, an inspector or the
manager only releases presentation subscriptions. A presentation or RPC failure
never removes the worker's destination directory; it remains usable for retry.
Native completion is authoritative: a late UI cancellation flag cannot delete an
already committed file. Other Quick Paste clipboard
actions remain usable while a file is downloading, and a superseded file action
cannot copy or paste after another activation. Explicit Cancel stops the
operation. Application shutdown interrupts work and leaves a durable interrupted
record; reopening a panel does not restart or cancel an active worker.

The local SQLCipher database records operation identity, file, direction, source
and destination labels, byte counts, phase, reason, times and local path. Typed
runtime updates share active state across engines. Secret file names are hidden
in operation history, including after a secrecy change; deferred peer readers
recheck sharing policy, deletion and active pairing access for each block. UI progress updates reuse
these signals without requerying the history per percentage. Records are paged
in groups of 50; only the current page, active operations and a bounded recent
clip-state cache remain resident. Clear history removes only terminal operation
records, including failures and cancellations; files, clips and active workers
remain. Previously known downloaded-file links migrate with an explicitly
unknown transfer time. Earlier transfers without a stored link cannot be
reconstructed.

Downloaded destinations belong to the user. History records their local paths
without synchronizing those paths as new versions or creating a second hidden
full-byte cache. Quick Paste uses a stable application-support destination and
reuses the existing native-recorded path on subsequent activations. The manager
provides a native Show saved file action. Copy verifies the destination against
the offer or the verified checksum retained for a deferred download; a file without
local bytes reports that it must be downloaded first. Local snapshots can be
copied without a network request. Native clipboard writers receive a file, never
the offer JSON or its display path as text. Scoped plaintext is streamed into
the existing native clipboard staging owner and removed after the write.

Settings exposes Maximum file size for local import/capture and explicit
download. The default is 64 MiB and the supported setting range is 1 MiB through
20 GiB. The text/image and ordinary sync-message limits remain separate. A lower
limit does not remove retained snapshots. File errors retain the actual runtime/platform explanation. Inspectors display
it above metadata, using the shared StateView; action notifications retain the
shared AppToast. Quick Paste separates its transfer status from its shortcut.
Size failures report the actual size and configured limit. User cancellation is
not an error toast.

The P2P message version is 8 and private IPC version is 6. All participating
devices must run the matching protocol. No protocol downgrade sends file bytes
through ordinary sync. Resource controls are registered as RUN-036 and RUN-056; they are
not proof of native resource qualification.
