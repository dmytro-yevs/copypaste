# Performance — what the always-running parts cost

**Question asked:** *a daemon polls a clipboard forever and a phone app will be
judged on battery. What does that cost, and where does the time go?*

Battery cannot be measured on this host. What is reported instead is the two
things that determine it — **wakeups per minute** and **CPU-seconds per hour of
idling** — plus a per-stage breakdown of the capture and read paths.

---

## 0. Method, and which numbers leave this host

| | |
|---|---|
| Host | Linux 6.18.5 container, 4 vCPU, 16 GB. Shared with other agents; every measurement records the 1-minute load average beside it. |
| Toolchain | `cargo 1.96.1`, `--release` for the daemon, `bench` profile for the microbenchmarks. |
| Harness | `criterion 0.8` (dev-dependency, `default-features = false`) for the microbenchmarks; `/proc` sampling for the daemon. `hyperfine`, `perf` and `/usr/bin/time` are not installed here — `/proc/<pid>/stat` and `/proc/<pid>/task/*/status` are the counters those tools read. |
| Clipboard backend | **the fake one.** `NSPasteboard` and Android's `ClipboardManager` do not exist here. |
| Not measured | Battery. Real pasteboard access latency. Anything on a phone. Wall-clock latency of the Tauri/React render. |

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
cargo +1.96 bench -p copypaste-core --bench capture   # (or history, detect)
```

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

**At the shipped default of 500 ms that is ≈745 wakeups per minute, 12 per
second, forever, on a clipboard nobody has touched.**

The fixed floor of ~14/min is the two sync loops (`p2p::poll` at
`NO_PEERS_INTERVAL` = 60 s, `cloud::poll` at `SIGNED_OUT_INTERVAL` = 60 s) plus
the r2d2 pool reaper. Both back off correctly and neither is a battery concern
at idle.
