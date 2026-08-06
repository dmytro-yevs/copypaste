# Performance — what the always-running parts cost

**Question asked:** *a daemon polls a clipboard forever and a phone app will be
judged on battery. What does that cost, and where does the time go?*

Battery cannot be measured on this host. What is reported instead is the two
things that determine it — **wakeups per minute** and **CPU-seconds per hour of
idling** — plus a per-stage breakdown of the capture and read paths.

---

## 0. Method, and which numbers leave this host

Two hosts appear below and they are not interchangeable. Every table names
which one it was taken on.

| | Host L | Host M |
|---|---|---|
| Machine | Linux 6.18.5 container, 4 vCPU, 16 GB | Apple M4, 10 cores, macOS 27.0 (Darwin 27.0) |
| Shared with | other agents; load recorded per measurement | five other agents building concurrently; load recorded per measurement |
| Toolchain | `cargo 1.96.1`, `--release` for the daemon, `bench` profile for the microbenchmarks | `cargo` stable aarch64, same profiles |
| Daemon counters | `/proc/<pid>/stat`, `/proc/<pid>/task/*/status` | `proc_pid_rusage` and `ps -M` (`scripts/profile/darwin-rusage.py`) — Darwin has no `/proc` |
| Clipboard backend | **the fake one.** `NSPasteboard` and Android's `ClipboardManager` do not exist there | the real `NSPasteboard` |

`criterion 0.8` (dev-dependency, `default-features = false`) is the
microbenchmark harness on both. Not measured on either: battery, anything on a
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

---

## 3. The text capture path, per stage — **T**, except `insert` which is **S**

Host M, `cargo bench --bench capture` and `--bench detect`, load 121 falling to
22 — the most contended table here, and the one to re-take first when a quiet
host is available. The ratios between stages are what it is for.

| stage | 256 B | 64 KiB | 4 MiB |
|---|---|---|---|
| detect | 2.84 µs | 207 µs | 15.0 ms |
| hash | 1.29 µs | 158 µs | 8.27 ms |
| encrypt | 2.90 µs | 140 µs | 7.05 ms |
| dedup probe | 7.87 µs | 2.60 µs | 2.30 µs |
| insert | 3.15 ms | 76.8 ms | 74.0 ms |
| **whole `ingest`** | **12.0 ms** | **266 ms** | **413 ms** |

Detection is the most expensive pure-computation stage at every size — 15.0 ms
at 4 MiB, twice the AEAD. The dedup probe is flat because it is an index seek,
not a scan.

**The stages do not add up to the total, and the gap is not yet explained.** At
256 B the measured stages sum to ~3.2 ms against a 12.0 ms `ingest`. The three
sweeps that ride every capture account for well under a millisecond of it at a
10 000-row history — cap 122 µs, byte cap ~0.4 ms extrapolated from §5, and
age-based retention is off by default. That leaves ~8 ms unattributed on a
table taken at load 65 against stage rows taken at load 22, which is the most
likely explanation and is not a measurement anyone should build on: **re-take
§3 on a quiet host before quoting the total.** The stage rows and the ratios
between them are the usable part.

Detection alone, `--bench detect`, load 121:

| | 64 B | 1 KiB | 116 KB | 1 MiB | 4 MiB |
|---|---|---|---|---|---|
| benign | 1.39 µs | 9.78 µs | 951 µs | 7.15 ms | 36.8 ms |
| matching | 3.33 µs | 15.0 µs | 1.16 ms | 25.0 ms | 148 ms |

Text that matches rules costs up to 4× text that does not, and the gap widens
with size: the prefilter is cheap, the per-rule searches behind it are not.

Two fixed costs, same run: `Detector::new()` **349 ms** (once per process,
never per call — `CopyPaste-mnte`), and `Store::open` **3.70 ms**.

The idle tick with the sensitive sweep off — the shipped default — is
**5.36 ns**. With it on it is 289 µs against a 2 000-row history.

---

## 4. The binary capture path — **T**

Host M, `cargo bench --bench binary`, load 22.6 falling to 9.5. Pure
computation plus one insert, so the class is T: an M4 is a different constant
from a phone, not a different shape.

| payload | `item_id` (one SHA-256) | `seal` (STREAM) | `open` | whole `ingest` |
|---|---|---|---|---|
| 256 KiB | 501 µs | 1.42 ms | 1.48 ms | 4.93 ms |
| 1 MiB | 2.09 ms | 6.13 ms | 3.79 ms | 22.7 ms |
| 4 MiB | **8.41 ms** | 23.4 ms | 16.3 ms | **91.3 ms** |

**SHA-256 here runs at 475 MiB/s**, not the ~2 GB/s an ARMv8 crypto-extension
path would give and not the 334 MB/s `openssl speed` reports for LibreSSL. That
was the open question behind the binary path's four-passes-over-one-payload
problem, and it settles it: at 4 MiB each pass is 8.4 ms, four passes are
33.6 ms, and the three redundant ones are **25.2 ms of a 91.3 ms capture — 28%
of the whole path**, on the foreground thread, per screenshot.

`capture/stage/4MiB/hash` measures the same primitive through the text path and
lands at 8.27 ms, which is the cross-check.

---

## 5. The store, keyed — **S**

Host M, `cargo bench --bench storage`, load 8.2 to 12.1. Class S: every page a
statement touches is an AES-256-CBC decrypt plus an HMAC-SHA512 verify, so
these are SQLCipher figures rather than SQLite ones, but the constant is this
machine's APFS and this machine's AES.

| rows | `summaries(i64::MAX)` | `insert_or_bump` (insert) | `insert_or_bump` (bump) | `upsert` |
|---|---|---|---|---|
| 500 | 622 µs | 374 µs | 7.9 µs | 570 µs |
| 2 000 | 2.49 ms | 984 µs | 7.5 µs | 843 µs |
| 8 000 | 8.88 ms | 1.92 ms | 8.1 µs | 3.99 ms |

`summaries` is linear at **1.11 µs per row** and it is the read a sync round
opens with, unbounded. The bump branch is flat and ~240× cheaper than the
insert branch at 8 000 rows: what a duplicate capture costs is not the store,
it is everything the caller did before reaching it.

The byte-cap sweep, with nothing to evict, which is what it does on all but one
capture in a history (load 17.8 falling to 9.4):

| rows | `evict_over_byte_cap`, nothing to do |
|---|---|
| 500 | 25.8 µs |
| 2 000 | 82.2 µs |
| 8 000 | **331 µs** |

Linear, because it sums the stored bytes of the whole table — inside a write
transaction it opened before it could know there was nothing to do. Every
capture and every applied remote item pays it. At the shipped 10 000-row
`history_limit` that extrapolates to ~0.4 ms each.

---

## 6. Merging a peer's session — **S**

Host M, `cargo bench --bench sync`, load 5.4 to 11.1. The unit is a whole
session of N applies against a primed history, because the per-item cost is the
question and it is invisible at N = 1.

| history | 10 items | per item | 200 items | per item |
|---|---|---|---|---|
| 500 | 21.2 ms | 2.12 ms | 180 ms | 0.90 ms |
| 2 000 | 35.5 ms | 3.55 ms | 342 ms | 1.71 ms |
| 8 000 | 63.3 ms | 6.33 ms | **1.28 s** | **6.41 ms** |

Read the last column downwards. A 200-item session costs 0.90 ms per item
against a 500-row history and 6.41 ms against an 8 000-row one — **the per-item
cost is proportional to the history, not to the item**. Read it across, and the
batching benefit that exists at 500 rows (0.90 against 2.12) has gone entirely
by 8 000 (6.41 against 6.33): whatever scales with the history is being paid
once per item and swamps everything a larger batch could amortise.

`sync/summaries` is the same read as §4's through the source layer: 491 µs,
1.57 ms, 6.96 ms at the three depths.

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
