# copypaste-sync

## Purpose
P2P clipboard sync engine. Drives the HELLO/HAVE/WANT/ITEMS/DONE handshake over any duplex stream, tracks per-peer Lamport clocks, and resolves conflicts with last-writer-wins (LWW).

## Public API
From `src/lib.rs`:

- `engine::{SyncEngine, SyncResult, SyncError, PeerState}` — orchestrator that runs a single session against a stream.
- `clock::LamportClock` — monotonic logical clock (`tick` / `observe`).
- `protocol::{Message, WireItem}` — wire protocol enum.
- `merge::{resolve, local_to_wire, wire_to_local, MergeOutcome}` — CRDT-ish LWW merge helpers.

The engine is transport-agnostic: pass any `tokio::io::DuplexStream` (TCP, TLS-wrapped, in-memory mock).

## Platform support
All platforms.

## Status
beta.

## Internal vs published
Internal workspace crate. Not published to crates.io.

## Quick example

```rust,no_run
use copypaste_sync::engine::SyncEngine;
use copypaste_core::storage::items::ClipboardItem;

# async fn example(mut stream: tokio::io::DuplexStream) -> Result<(), Box<dyn std::error::Error>> {
let mut engine = SyncEngine::new("my-device-uuid");
let local_items: Vec<ClipboardItem> = vec![]; // load from DB
let (result, to_upsert) = engine.run_session(&mut stream, &local_items).await?;
println!("received {} new items", result.items_received);
# Ok(())
# }
```

## Tests
5 integration tests under `tests/`: conflict resolution, CRDT behaviour, handshake, Lamport clock, protocol-version negotiation.

```bash
cargo test -p copypaste-sync
```
