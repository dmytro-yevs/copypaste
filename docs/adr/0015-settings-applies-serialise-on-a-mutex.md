# ADR-0015: Serialise settings applies on a mutex, not on the read lock

Status: accepted

## Decision

`Settings::apply` takes a private `Mutex<()>` for the whole read-modify-write
and takes `RwLock::write` only to publish the new value. The SQLCipher write
that persists the record runs between the two, under no reader-visible lock.

`arc-swap` was evaluated and not added. It replaces the read side, not the
finding: an `ArcSwap<ConfigData>` still needs a mutex around
read-validate-persist-publish, or two overlapping patches lose one another's
fields — the defect the old write lock was there to prevent. What it adds on
top of that mutex is wait-free reads in place of a lock held across one move,
and `Arc` bumps in place of the seven `ConfigData` clones its callers make.

Cost of taking it, stated rather than assumed: one small crate with no required
dependencies, pure Rust on both targets, negligible build and binary impact.
Against that, `Settings::get` changes its return type, which is a mechanical
edit at every reader in the daemon — capture, notify, both sync transports and
the server modules. That is a wide diff on a tree whose first full build has
not been run, bought for a finding rated low. Reconsider it if the read side
becomes hot; nothing here forecloses it.

## Consequences

A settings change no longer blocks `Settings::get` for the duration of a
database write. The tail is what mattered, not the mean: `set_state` waits on
an r2d2 checkout (10 s timeout) and then on SQLite's write lock (5 s
`busy_timeout`), and two of the readers it blocked run on the tokio reactor
thread in `capture::run`.

Readers now see the new value shortly after the record reaches disk rather than
at the same instant. Persist-before-publish is unchanged, and remains the
ordering the module is built around.

Patches stay serialised: `applying` is held across validate, persist and
publish, so no two applies can read the same "before".
