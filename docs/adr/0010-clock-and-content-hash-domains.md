# ADR-0010: Share only identical clock and content-hash domains

Status: accepted

## Decision

`copypaste-p2p::protocol::plaintext_content_hash` owns the full lowercase
SHA-256 identity used on the P2P wire. Core's plaintext dedup hash is an alias
of that function because the merge contract requires those values to be
identical. Cloud rows carry no content hash; after authenticated decryption the
daemon computes the same plaintext identity through the core alias.

`copypaste-clock` owns the wall clock: `WallClock` for the injection shared by
core, P2P, and the daemon refresh loop, and `now_ms`/`unix_millis` for the
callers that only ever read the real clock — core, P2P, cloud, and the CLI. It
is not an elapsed-time clock. Deadlines, cadence, uptime, and debounce continue
to use `Instant` or `tokio::time`.

Superseded here: the cloud and CLI conversions previously stayed local, the
cloud one because it saturated above `i64::MAX` and the shared one did not. That
was a divergence, not a domain boundary. The shared conversion now saturates at
both ends, which is what the cloud crate already needed and what core and P2P
silently wanted: `as i64` on a `u128` wraps, so a clock far enough ahead
produced a *negative* stamp that sorts as older than every real item and inverts
every expiry comparison. The CLI depends on `copypaste-clock` for it; the crate
has no dependencies of its own, so the IPC-only client gains no runtime.

Binary-envelope SHA-256 remains inside the versioned envelope implementation:
it supplies a truncated item id and authenticated header bytes, not only the
sync identity. Cloud HMAC-SHA256 and HKDF-SHA256 remain in their keyed,
domain-separated security contexts. Sharing any of those implementations would
couple values with different version, wire, or key semantics.

## Consequences

Plaintext dedup and P2P identity cannot drift, and refresh tests inject wall
time deterministically. No serialized bytes, security domain, or monotonic
timing behavior changes. Core and P2P stamps saturate rather than wrap at the
far end of the range; no reachable clock produces a different value.
