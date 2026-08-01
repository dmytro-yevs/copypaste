# ADR-0010: Share only identical clock and content-hash domains

Status: accepted

## Decision

`copypaste-p2p::protocol::plaintext_content_hash` owns the full lowercase
SHA-256 identity used on the P2P wire. Core's plaintext dedup hash is an alias
of that function because the merge contract requires those values to be
identical. Cloud rows carry no content hash; after authenticated decryption the
daemon computes the same plaintext identity through the core alias.

`copypaste-clock::WallClock` owns the wall-clock injection shared by core, P2P,
and the daemon refresh loop. It is not an elapsed-time clock. Deadlines,
cadence, uptime, and debounce continue to use `Instant` or `tokio::time`.

The cloud wall-clock conversion stays local because it saturates values above
`i64::MAX`; changing it to the core/P2P conversion would change behavior. The
CLI clock also stays local so the IPC-only client does not gain a runtime
dependency for display timestamps.

Binary-envelope SHA-256 remains inside the versioned envelope implementation:
it supplies a truncated item id and authenticated header bytes, not only the
sync identity. Cloud HMAC-SHA256 and HKDF-SHA256 remain in their keyed,
domain-separated security contexts. Sharing any of those implementations would
couple values with different version, wire, or key semantics.

## Consequences

Plaintext dedup and P2P identity cannot drift, and refresh tests inject wall
time deterministically. No serialized bytes, security domain, or monotonic
timing behavior changes.
