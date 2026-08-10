# Performance — what the always-running parts cost

**Question asked:** *a daemon polls a clipboard forever and a phone app will be
judged on battery. What does that cost, and where does the time go?*

Battery cannot be measured on this host. What is reported instead is the two
things that determine it — **wakeups per minute** and **CPU-seconds per hour of
idling** — plus a per-stage breakdown of the capture and read paths.

---

## 0. Method, and which numbers leave this host

Three hosts appear below and they are not interchangeable. Every table names
which one it was taken on.

| | Host L | Host M | Host W |
|---|---|---|---|
| Machine | Linux 6.18.5 container, 4 vCPU, 16 GB | Apple M4, 10 cores, macOS 27.0 (Darwin 27.0) | Windows 11 Pro 10.0.26200, Intel i7-13700K, 16 cores / 24 threads, 64 GB |
| Shared with | other agents; load recorded per measurement | five other agents building concurrently; load recorded per measurement | other Orca worktrees; no core isolation |
| Toolchain | `cargo 1.96.1`, `--release` for the daemon, `bench` profile for the microbenchmarks | `cargo` stable aarch64, same profiles | `cargo 1.97.1` x86_64 MSVC, bench profile |
| Daemon counters | `/proc/<pid>/stat`, `/proc/<pid>/task/*/status` | `proc_pid_rusage` and `ps -M` (`scripts/profile/darwin-rusage.py`) — Darwin has no `/proc` | not used |
| Clipboard backend | **the fake one.** `NSPasteboard` and Android's `ClipboardManager` do not exist there | the real `NSPasteboard` | not exercised by the storage measurement |

`criterion 0.8` (dev-dependency, `default-features = false`) is the
microbenchmark harness on all three. Not measured: battery, anything on a
phone, wall-clock latency of the Tauri/React render.

**The Darwin wakeup counter is not the Linux one.** Host L counts context
switches summed over `task/*`; host M counts `ri_interrupt_wkups +
ri_pkg_idle_wkups`, which is what macOS's own battery accounting counts and
excludes a voluntary switch that woke nothing. A wakeups/min figure from one
host must never be put in a table beside the other's.

### Transferability, stated per number

Three classes, marked on every table below:

* **T — transfers.** Pure computation over the same inputs: AEAD, SHA-256,
  the regex engine, SQLite. An arm64 Mac or phone will be a different constant,
  not a different shape, and the *ratios* between stages hold.
* **S — shape transfers, constant does not.** Anything touching the filesystem
  or the scheduler. A phone's flash and a Mac's APFS are not this container's
  ext4-on-virtio.
* **F — fake-backend only.** Anything whose cost is `FakeClipboard`'s. These
  are **not** macOS or Android numbers and must not be quoted as such.

The most important **F**: the idle tick's clipboard read here is a `stat(2)` on
a file, or nothing at all. On macOS it is an Objective-C message to
`NSPasteboard.changeCount`. The *number of wakeups per minute* transfers,
because it is a property of the loop structure, not of the backend. The
CPU-per-tick does not.

### Controlling for a busy host

There is no isolated core here and six other agents build concurrently.
`scripts/profile/quiet-run.sh` waits for the 1-minute load average to fall
below a threshold before it starts, gives up after a bound, and prints the load
before and after either way. Every number below carries the load it was taken
at. CPU-counter measurements (the idle figures) are process-attributed and
barely move under contention; the criterion latencies are the ones to distrust,
and their variance is reported.

---

## 1. How to re-run this

```sh
scripts/profile/all.sh                          # everything, into target/profile-*.log
NO_MDNS=1 scripts/profile/daemon-idle.sh 300 500  # the daemon's own idle cost
scripts/profile/daemon-soak.sh 8 4 512          # 8 min at 4 captures/s of 512 B
scripts/profile/ipc-cost.sh 10000 500           # per-request daemon CPU
cargo bench -p copypaste-core --bench capture   # or binary, history, storage, sync, detect
cargo bench -p copypaste-p2p   --bench session  # one peer round, both directions
```

The profile scripts run on Linux and on macOS; `lib.sh` branches on `uname`
and the two sides read different counters — see §0. The daemon they start is
isolated by `--data-dir` and `COPYPASTE_SOCKET`, never by `XDG_DATA_HOME`
alone, because `directories::ProjectDirs` ignores XDG on macOS and would
otherwise have profiled against the user's real history. On macOS the device
secret still comes from the login Keychain, which a profiling run shares with a
real install: it may raise a Keychain prompt, and nothing here deletes it.

Benches are `harness = false` and are not built by `cargo test`; that was
checked with `cargo test -p copypaste-core --no-run`, which lists only the lib
test binary.

**Always pass `NO_MDNS=1` for an idle number.** See §2.1: with LAN visibility
on, the browse thread's wakeups are a function of how many other CopyPaste
daemons are advertising on the segment, and on a shared build host that is
several — the daemon then reports their traffic as its own idle cost.

---

## 2. The idle daemon — the battery proxy

`poll_interval_ms` defaults to **500**. Everything below is a daemon with an
empty clipboard, no peers, no cloud account and no client connected.

### 2.1 Wakeups per minute — **T for the rate, F for the per-tick work**

The count of wakeups is a property of the loop's structure, so it transfers to
macOS and Android; what each wakeup *does* here is the fake backend's work and
does not.

Run 1, LAN visibility **on**, host contended (this is the run that shows why
§1's `NO_MDNS=1` note exists):

| `poll_interval_ms` | host load | total wakeups/min | of which `mDNS_daemon` | remainder |
|---|---|---|---|---|
| 500 | 10.98 | 2103.7 | 1376.6 | **727.8** |
| 1000 | 1.43 | 865.0 | 504.0 | **361.2** |
| 5000 | 0.64 | 109.8 | 22.8 | **87.0** |

The `mDNS_daemon` column tracks the **host load**, not the poll interval —
other agents' daemons and `demo-p2p.sh` runs were advertising `_copypaste._tcp`
on the same segment, and every advertisement wakes the browse thread. That is a
measurement artifact here, but it is also a real property: on a LAN with many
CopyPaste devices, discovery is a wakeup source the poll interval does not
control.

The remainder is linear in the tick rate:

```
wakeups/min ≈ 6.1 × ticks/min + ~14
```

with 6.07, 6.02 and 7.25 wakeups per tick measured at the three intervals (the
last includes the fixed floor). **Six wakeups per clipboard poll.** One tick is
a timer fire, a `spawn_blocking` handoff to the blocking pool, the blocking
thread's own wake, and the completion waking the awaiting task again — the
`tokio-rt-worker` threads carry 92% of the remainder, and `r2d2-worker-*`
contributes a flat ~11/min regardless of interval.

**At the shipped default of 500 ms that was ≈745 wakeups per minute, 12 per
second, forever, on a clipboard nobody has touched.**

### What was done about it

The handoff was unconditional: every tick went to the blocking pool to ask a
question whose answer was almost always "nothing changed". `ClipboardSource`
now answers that on the async side (`changed()`, a bare `changeCount` read),
and only a tick with something to do is handed off. The sensitive-item sweep
used to ride the poll and so moved onto its own cadence, and does not run at
all while `sensitive_ttl_secs` is `0`, which is the shipped default.

Re-measured on the same harness, `NO_MDNS=1`, 500 ms, load 2.67:

| | wakeups/min |
|---|---|
| before | 745 |
| after | **258.7** |

CPU at idle after the change is 1.2 CPU-s/hour; there is no comparable
before-figure to put beside it, so none is quoted.

≈2 wakeups per tick, against 6.1 before — a timer fire and the worker it wakes,
which is the floor for a polled loop. `changed()` defaults to `true`, so a
backend that cannot answer cheaply is polled exactly as before.

The fixed floor of ~14/min is the two sync loops (`p2p::poll` at
`NO_PEERS_INTERVAL` = 60 s, `cloud::poll` at `SIGNED_OUT_INTERVAL` = 60 s) plus
the r2d2 pool reaper. Both back off correctly and neither is a battery concern
at idle.

### 2.2 The same daemon on macOS — **first measurement on a shipping platform**

Everything above §2.2 is host L with the fake clipboard backend. This is host M
with the real `NSPasteboard`, `NO_MDNS=1`, `poll_interval_ms` 500, load 24.7 —
contended, and stated as such.

| | 300 s window | 60 s window |
|---|---|---|
| Wakeups | **281.7/min** | 281.8/min |
| CPU | 0.001 s ⇒ **0.012 CPU-s/hour** | below the counter's resolution |
| RSS | 103.6 MB → 96.9 MB | 104.8 MB → 104.7 MB |
| Threads | **17** | 17 |

Three things this says that host L could not.

The **wakeup rate is 281.7/min against host L's 258.7**, and the two counters
are not the same counter (§0), so the honest reading is "the same order, on the
platform that ships" rather than a difference of 23.

The **CPU is two orders of magnitude below host L's 1.2 CPU-s/hour**, and that
is the fake backend's absence: a `changeCount` read is a few microseconds, and
120 of them per minute is 0.4 ms. The wakeups, not the CPU, are what a battery
will notice.

**17 threads at idle** is the figure to watch. It is the whole reason this
section exists: a thread that only exists on macOS was never in any number
taken on host L.

**Not re-taken after wave 1, and it should be.** F-IDLE-1 made the paste-file
staging sweeper start on the first paste-back instead of at construction, and
lengthened its interval from 30 s to 10 min; F-IDLE-2 returns from the cloud
refresh loop immediately when no deployment is configured, removing a 10 s
tick. Both remove threads or timers from exactly this measurement, so 281.7/min
and 17 threads are a **pre-wave** figure. Re-taking it needs an interactive
approval: the daemon reads the device secret from the login Keychain, and a
freshly built binary blocks on the access prompt, so `daemon-idle.sh` cannot
complete unattended — except that the profiling wrapper builds the daemon with
`dev-ephemeral-key`, after which `COPYPASTE_EPHEMERAL_KEY` short-circuits the
keystore before any Security-framework call. Shipped builds do not contain
that runtime path, and a daemon profiled on a throwaway `--data-dir` never
needed the real secret.
[macos-idle-measurement.md](macos-idle-measurement.md) has the diagnosis, that
route and three alternatives, and what the re-taken figures would falsify;
`scripts/profile/macos-keychain-preflight.sh` decides which route this Mac
needs and `scripts/profile/macos-idle-after.sh` runs it.

---

## 3. The text capture path, per stage — **T**, except `insert` which is **S**

Host M, `cargo bench --bench capture` and `--bench detect`, load 3.3 to 6.3.
Re-taken on a quiet host after the wave-1 merge; the earlier version of this
table was captured at load 65-121 and every number in it moved.

| stage | 256 B | 64 KiB | 4 MiB |
|---|---|---|---|
| detect | 1.08 µs | 141 µs | 9.76 ms |
| hash | 0.68 µs | 115 µs | 7.61 ms |
| encrypt | 1.38 µs | 111 µs | 6.95 ms |
| dedup probe | 4.10 µs | 3.58 µs | 3.37 µs |
| insert | 294 µs | 1.48 ms | 103 ms |

Detection is still the most expensive pure-computation stage at every size, and
at 4 MiB it is 1.4× the AEAD. The dedup probe is flat because it is an index
seek, not a scan.

**`insert` at 64 KiB was 57.4 ms before this wave and is 1.48 ms now.** That is
F-STOR-1: every FTS write first deleted the row's old index entry by `id`,
which `clipboard_fts` cannot seek on, so each insert full-scanned the plaintext
index. At 4 MiB `insert` moved the other way, 67.6 ms to 103 ms, because the
`fts_rowid` back-pointer is written by a second `UPDATE` of a row that holds
the whole ciphertext.

**The whole path is not the sum of these stages, and the difference is not
explained.** `ingest` measures 2.55 ms at 256 B, 4.80 ms at 64 KiB and 222 ms
at 4 MiB, against stage sums of 0.30 ms, 1.85 ms and 127 ms. The gap is roughly
constant at the two small sizes and large at 4 MiB. Load is not the explanation
— this run is quiet — so quote the stage rows and the ratios between them, and
do not quote the totals as a decomposition until someone accounts for the
remainder.

Detection alone, `--bench detect`:

| | 64 B | 1 KiB | 116 KB | 1 MiB | 4 MiB |
|---|---|---|---|---|---|
| benign | 362 ns | 2.94 µs | 230 µs | 2.07 ms | 8.33 ms |
| matching | 1.14 µs | 4.69 µs | 280 µs | 2.49 ms | 10.02 ms |

Text that matches rules costs about 1.2× text that does not. The earlier 4×
figure was load, not detection. This group is a no-regression check and nothing
more: `benign` has no matches, so the floor-membership filter F-CORE-4 added
never bites, and `matching` fires a rule above the floor immediately, so it
cannot skip anything either. F-CORE-4's own number is the in-process A/B in
`the_predicate_is_cheaper_than_the_ranked_scan_it_replaced`.

`Detector::new()` is **102 ms**, once per process and never per call
(`CopyPaste-mnte`). `capture/store_open` is not quoted: the group aborts on
this host because it opens a fresh pool per iteration and runs out of threads,
on this tree and on the pre-wave one alike.

The idle tick with the sensitive sweep off — the shipped default — is
**3.33 ns**. With it on it is 218 µs against a 2 000-row history.

---

## 4. The binary capture path — **T**

Host M, `cargo bench --bench binary`, load 3.7 to 5.3, run on the pre-wave tree
and on the merged one back to back. Pure computation plus one insert, so the
class is T: an M4 is a different constant from a phone, not a different shape.

| 4 MiB | before | after |
|---|---|---|
| `item_id` (one SHA-256) | 7.66 ms | 8.23 ms |
| `seal` (STREAM) | 22.32 ms | **14.47 ms** |
| `open` | 15.02 ms | 15.04 ms |
| whole `ingest` | 77.97 ms | **55.62 ms** |

| whole `ingest` | before | after |
|---|---|---|
| 256 KiB | 4.50 ms | 3.03 ms |
| 1 MiB | 17.68 ms | 11.53 ms |
| 4 MiB | 77.97 ms | 55.62 ms |

**SHA-256 here runs at about 535 MiB/s**, not the ~2 GB/s an ARMv8
crypto-extension path would give. That is why the four-passes-over-one-payload
problem was worth fixing and it is why the hardware backend is still worth
enabling: at 4 MiB one pass is 7.7 ms.

F-CORE-1 threaded a single digest through the item id, the envelope header and
the row's `content_hash`. **Measured: 22.3 ms off a 78 ms capture, 29% of the
path**, on the foreground thread, per screenshot. `seal` accounts for 7.9 ms of
that (it hashed the payload twice internally) and the two removed callers for
the rest.

F-CORE-5 replaced STREAM's per-chunk allocate-and-copy with an in-place seal and
open against a presized output buffer. **On wall time it is a null result** —
`open` at 4 MiB is 15.02 ms before and 15.04 ms after. What it removes is
allocation: one `Vec::with_capacity` for the whole plaintext instead of one
`Vec` per chunk plus the growth reallocations of a `Vec::new()`. That count is
not measured here.

---

## 5. The store, keyed — **S**

Host M, `cargo bench --bench storage`, load 3.7 to 5.3, both trees. Class S:
every page a statement touches is an AES-256-CBC decrypt plus an HMAC-SHA512
verify, so these are SQLCipher figures rather than SQLite ones.

| 8 000 rows | before | after |
|---|---|---|
| `summaries(i64::MAX)` | 5.25 ms | **1.39 ms** |
| `insert_or_bump` (insert) | 1.174 ms | **264 µs** |
| `insert_or_bump` (bump) | 5.24 µs | 4.71 µs |
| `upsert` | 2.142 ms | **277 µs** |
| `evict_over_byte_cap`, nothing to do | 202 µs | 166 µs |

`summaries` is the read a sync round opens with, unbounded; `idx_items_syncable`
makes it a covering seek instead of a scan plus a temp B-tree. `upsert` and
`insert` both fell because every FTS write used to delete the row's old index
entry by `id`, which `clipboard_fts` cannot seek on.

**The byte-cap sweep is the one place the wave argued for a win and did not get
one.** F-STOR-3 moved the gate query out of the IMMEDIATE transaction so a sweep
with nothing to do would not take the write lock. Measured, that changed
nothing: 205 µs against 208 µs at 8 000 rows uncontended, and 0.22 ms per
capture on both trees with a rate-matched writer running concurrently. The gate
query — a `SUM(LENGTH(...))` over every unpinned row — is the whole cost, and
hoisting it does not remove it. **F-STOR-3 was reverted.** The 202 µs to 166 µs
above is what is left, and the only candidate for it is F-STOR-4's
`PRAGMA optimize`.

### 5.1 Retention and payload sweeps — **Windows target baseline**

The depth sweep above holds rows at 512 bytes, so it cannot expose a change
whose cost is per byte rather than per row. `storage/payload` holds depth at 32
and sweeps 512 B, 64 KiB, 1 MiB and 4 MiB through `insert`, `bump`, `upsert`
and the byte-cap gate. The two separate axes avoid an unaffordable matrix:
8 000 four-MiB rows would require 32 GB before FTS duplication and encryption.

```sh
cargo bench -p copypaste-core --bench storage -- 'storage/(sweep|payload)'
```

Host W, 2026-08-09. These are medians from the configured Criterion runs; its
reported 95% confidence intervals bracket every value below.

| retention gate, nothing to do | 500 rows | 2 000 rows | 8 000 rows |
|---|---:|---:|---:|
| item cap | 12.010 µs | 12.589 µs | 12.140 µs |
| byte cap | 39.759 µs | 105.81 µs | 340.69 µs |

The item-cap gate stays within 4.8% while history grows 16×. It now reads the
transactionally maintained singleton instead of counting live rows. The byte
cap remains the control: its `SUM(LENGTH(...))` still scales with row count.

| 32-row payload sweep | 512 B | 64 KiB | 1 MiB | 4 MiB |
|---|---:|---:|---:|---:|
| insert | 1.2802 ms | 5.1240 ms | 49.124 ms | 245.88 ms |
| bump | 18.886 µs | 57.143 µs | 1.8097 ms | 6.0287 ms |
| upsert | 1.3672 ms | 4.9035 ms | 80.358 ms | 286.53 ms |
| byte-cap gate | 22.476 µs | 17.615 µs | 17.502 µs | 17.811 µs |

The write paths scale with payload size and the byte-cap gate does not: SQLite
obtains a blob's length from the record header rather than reading its overflow
pages. This is the first release-target payload baseline; the earlier WSL2 run
was only a smoke test.

---

## 5.2 Late-sealed duplicate inserts — **no after-measurement**

`insert_or_bump_late_sealed` evaluates the sealing closure only after the
transaction has taken the dedup decision. A re-copy therefore avoids HKDF,
XChaCha20-Poly1305 and the plaintext clone; the content hash, sensitivity flag
and AAD-bound item id stay eager because the decision needs them.

Only the before-baseline exists, measured at p50 against 2,000 rows on a
contended host: 351 µs at 256 B, 365 µs at 4 KiB, 1.448 ms at 64 KiB and
24.1 ms at 1 MiB. The saving is argued from work the duplicate path no longer
executes, not observed; take an after-measurement before quoting a speedup.

---

## 6. Merging a peer's session — **S**

Host M, `cargo bench --bench sync`, load 3.7 to 5.3, both trees. The unit is a
whole session of N applies against a primed history, because the per-item cost
is the question and it is invisible at N = 1.

| 200 items | before | per item | after | per item |
|---|---|---|---|---|
| against 500 rows | 143 ms | 0.72 ms | 137 ms | 0.69 ms |
| against 2 000 rows | 305 ms | 1.53 ms | 209 ms | 1.04 ms |
| against 8 000 rows | **987 ms** | **4.93 ms** | **467 ms** | **2.34 ms** |

Read the per-item columns downwards. Before the wave the cost grew 6.8× as the
history grew 16×; after it grows 3.4×. The session against an 8 000-row history
is **2.1× faster**, and a 10-item session against the same history went 48.9 ms
to 24.9 ms.

The remaining growth is real: the debounce (F-CORE-3) removed the retention
sweep pair from every applied item, and reading the local version once instead
of twice (F-CORE-2 part i) removed one full-row read with its ciphertext, but
the `upsert` itself still scales with the history. F-CORE-2 part (ii) — reading
the metadata projection rather than the whole row — is wave 2.

`sync/summaries` at the three depths: 299 µs, 1.35 ms, 5.53 ms before;
**128 µs, 496 µs, 1.96 ms** after.

---

## 7. One peer sync round — **T for the bytes, S for the time**

Host M, `cargo bench -p copypaste-p2p --bench session`, load 9.5 to 12.8. Both
devices hold the same history, so nothing is fetched and nothing is applied:
this is the round a converged pair performs every tick to discover there is
nothing to do. The duplex is an in-process channel — the real encoder, the real
decoder and the real protocol bounds, but no TCP, no Noise and no LAN.

| summaries each side | bytes across the duplex, per round | wall |
|---|---|---|
| 100 | 51 732 | 324 µs |
| 1 000 | 514 332 | 2.73 ms |
| 10 000 | **5 140 332** | 46.1 ms |

**514 bytes per summary per round**, exactly linear, both directions counted.
The byte column is deterministic and does not depend on the host or its load;
only the wall column does.

At `MIN_POLL_INTERVAL` = 5 s, a converged pair at the default `history_limit`
of 10 000 moves **61.7 MB per minute, per peer, to learn nothing** — that
figure is arithmetic over a measured byte count, and the cadence is the
constant to check before quoting it.
