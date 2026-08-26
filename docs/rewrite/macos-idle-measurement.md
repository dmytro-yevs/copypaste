# The macOS idle-daemon AFTER measurement

**Status:** open · nothing below has been observed · the before-figures in
[performance.md](performance.md) §2.2 are pre-wave and stay pre-wave until this
runs
**Was blocking:** `scripts/profile/daemon-idle.sh` could not start a freshly
built daemon on a Mac holding a device-secret Keychain item. The profiling
wrapper now builds an explicit development-only bypass.

## What blocks, exactly

Five links. Each is in the tree.

1. `daemon-idle.sh` calls `build_release` before it measures anything, so every
   run measures a binary compiled seconds earlier (`scripts/profile/lib.sh`).
2. The daemon reads its device secret on the way up, before the socket exists
   and unconditionally, through `security_framework::passwords::generic_password`
   against the login Keychain (`crypto/keystore/macos.rs`, `daemon/src/main.rs`).
   `--data-dir` relocates the database and the socket; it does not relocate the
   Keychain item, because the service and account are frozen by port manifest
   02, I-10.
3. An ad-hoc or unsigned binary's designated requirement **is its cdhash**, so
   the ACL on the item names the previous build and not this one. Manifest 02
   §3.8 records the cdhash/ACL constraint. `scripts/release/smoke-macos-dmg.sh`
   re-signs the installed daemon and restarts it for the same reason, and
   reports rather than asserts the answer.
4. The read **is** bounded: `Keyring::load_or_create` runs it through
   `load_with_timeout(8 s)` on a dedicated `keystore-load` thread
   (`crypto/keys.rs`). So the daemon does not hang. It gets
   `KeystoreUnavailable("the keychain read timed out")`, and `halt_or_fail`
   binds the socket anyway and serves one refusal to every request
   (`server/halted.rs`). `copypaste status` fails against that, so `lib.sh`'s
   readiness poll runs out its 100 × 0.2 s and `die`s.
5. So the reported symptom — "daemon did not become ready" — is not a hang and
   is not a crash. It is a live daemon serving refusals, with a **detached**
   `keystore-load` thread still parked on the prompt: `load_with_timeout` drops
   the join handle rather than cancelling, because Security-framework exposes no
   cancellation.

`load_with_timeout` remains the production I-22 path. The environment bypass is
compiled only by `dev-ephemeral-key`; the profiling wrapper enables it before
starting the throwaway daemon.

CI does not hit any of this because it changes the default keychain
(`ci.yml`, "A throwaway keychain to test against"), and it must:
`security_framework`'s passwords API names no keychain, so `SecItemAdd` and
`SecItemCopyMatching` resolve the **default keychain and the user search list**,
both user-domain preferences with no per-process override. That is why the CI
workaround is the shape it is, and why it must not be copied onto a machine
somebody uses.

`scripts/profile/macos-keychain-preflight.sh` establishes all of this on the
Mac without changing anything: it separates *ready*, *halted*, *exited* and
*blocked*, and it runs the same probe again under the bypass below.

## The route: `COPYPASTE_EPHEMERAL_KEY`

With `dev-ephemeral-key` compiled, `Keyring::load_or_create` short-circuits on
`COPYPASTE_EPHEMERAL_KEY` **before any Security-framework call**, minting a
throwaway secret for the process lifetime. It is read exactly once into a
`OnceLock` so a mutated environment cannot flip an already-keyed daemon (port
manifest 02, I-23). Shipped builds compile this branch and the environment read
out entirely.

It is the right trade **for this measurement specifically**: the daemon is
started on a `mktemp -d` data directory whose history is discarded at the end of
the run, so which 32 bytes opened it was never part of the question. The idle
cost is loops and timers, and none of them touch the keyring.

**Why it does not move the thread count.** The comparison with §2.2's 17 threads
only holds if the bypass adds or removes no thread at idle. It does not:
`load_with_timeout` spawns `keystore-load`, and on a successful read
`worker.join()` reaps it before `load_or_create` returns — long before the
daemon is ready, let alone 2 s settled and 300 s sampled. §2.2's before-run
produced numbers, so its read succeeded, so its worker was already gone. Under
the bypass no such thread is spawned at all. **This is an argument from the
code, not an observation** — and it has a falsifier: a run reporting **18**
threads is a *timed-out* read whose worker is still parked on a prompt. Discard
that run.

What the bypass gives up is the real Keychain at startup, and that is not what
this measurement is for. `ci.yml`'s macOS job covers the Keychain on every push.

## Three alternatives, if the real Keychain path must be exercised

**A — a throwaway macOS user account.** A second local user has its own login
keychain, its own default-keychain preference and no CopyPaste item, so the run
mints its own secret and nothing prompts. Log in through fast user switching,
not `su`: the measurement needs a real `NSPasteboard` and a background session
has no pasteboard server, which would silently change the number. Record
`vm.loadavg` — a switched-out session is extra load.

**B — let the run mint its own item** (`KEYCHAIN_ROUTE=mint-fresh`). A write
creates an ACL rather than consulting one, so minting never prompts however
often the binary is rebuilt. Only available on a Mac with no CopyPaste history:
deleting that item leaves an installed history permanently unopenable, the
outcome AGENTS.md rule 4 ranks worst. The script refuses unless the database is
absent *and* `NO_COPYPASTE_INSTALL=1` says so deliberately, and removes what it
minted on the way out.

**C — a stable code identity** (`KEYCHAIN_ROUTE=signed`). `packaging/macos/selfsign.sh`
already keeps a per-machine certificate whose designated requirement does not
move between builds; ADR-0001 argues from Apple's own source that a TCC grant
held against it survives an update. If a Keychain ACL keys on the designated
requirement the same way, one "Always Allow" outlives every later rebuild.
**Nobody has observed that**, so this route is also the experiment — and it is
the one that would settle testing-policy's "The Keychain item survives a
re-signed binary (manifest 02 §3.8)". A second prompt after a rebuild refutes
it.

A second production key backend is not an alternative. The shipped path is the
Keychain; the ephemeral key exists only behind the development measurement
feature.

## What the run must hold fixed

`scripts/profile/macos-idle-after.sh` refuses rather than warns on each of
these, because a violated one makes the comparison with §2.2 void rather than
noisy:

* `poll_interval_ms` 500 and a 300 s window — §2.2's column.
* `NO_MDNS=1`. With LAN visibility on, the browse thread reports other daemons'
  advertisements as this daemon's idle cost (§2.1).
* No `COPYPASTE_CLOUD_URL` and no `COPYPASTE_CLOUD_ANON_KEY`. F-IDLE-2's early
  return is keyed on `is_configured`; with a deployment reachable, the branch
  under measurement is not the branch that runs.
* The default keychain and search list are read before and after and compared.

## What the result would falsify

The predictions are arithmetic on `f09f7334`, not measurements. F-IDLE-1 stopped
the paste-file sweeper being spawned at construction and lengthened its interval
from 30 s to 10 min, so an idle daemon should carry one fewer OS thread and one
fewer 30 s timer. F-IDLE-2 returns from cloud refresh before its loop when
nothing is configured, removing a 10 s tick — six timer expiries a minute,
against the ≈2 wakeups per tick §2.1 measured.

| Observation | What it falsifies |
|---|---|
| Threads still 17 | F-IDLE-1 did not take on macOS. The strongest single assertion in the run — the sweeper is the one thread whose presence is decidable from a count. |
| Threads 18 | A timed-out Keychain read left `keystore-load` parked on a prompt. The run is contaminated; discard it and use the bypass. |
| Wakeups ≥ 281.7/min | Neither fix reached the idle path, or something else regressed. Two removals cannot raise the number. |
| Wakeups ≈ 268–274/min | Both fixes took, and the honest conclusion is that they are *small*: the idle wakeup budget is dominated by the 500 ms clipboard poll at ≈2 wakeups a tick, not by either of them. |
| Wakeups well below 250/min | Something beyond these two moved. Do not credit them without the per-thread table `daemon-idle.sh` already prints. |
| CPU materially above 0.012 CPU-s/hour | A regression on the real `NSPasteboard` path. §2.2's figure is at the counter's floor. |

**The failure that would look like success.** A daemon whose keyring failed
still binds the socket and serves refusals (`server/halted.rs`), and a halted
daemon runs no capture loop, no sync loops and no sweeper — so it would report a
spectacular idle improvement. `copypaste status` fails against it, which is what
stops `lib.sh` measuring one, and `macos-idle-after.sh` probes readiness once
before committing to the 300 s window rather than inheriting that protection by
accident. If a future change makes `status` answer from the halted server, this
measurement silently becomes fiction.

The per-thread column on Darwin is **CPU time, not wakeups** (`lib.sh` says so,
and says why). `ps -M` carries no thread names, so the vanished sweeper cannot
be identified by name from the harness; `/usr/bin/sample <pid> 1` on the running
daemon names threads and is the cheap way to confirm `paste-file-cleanup` is
absent rather than merely uncounted. If `sample` is refused, the count is the
assertion.
