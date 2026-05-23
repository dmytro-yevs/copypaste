# copypaste-supabase

## Purpose
Optional Supabase backend for CopyPaste: GoTrue authentication and Realtime WebSocket sync. Implements the Phoenix Channel protocol manually because no official Rust Supabase SDK with Realtime support exists.

## Public API
From `src/lib.rs`:

- Auth — `AuthClient`, `Session`, `User`, `AuthError`, `AuthResult`, `SessionStore`, `InMemoryStore`.
- Realtime — `RealtimeClient`, `RealtimeConfig`, `ClientHandle`, `RealtimeError`.
- Protocol — `PhoenixMessage`, `PhoenixEvent`, `ChangeEvent`, `ChangeType`.

`AuthClient::from_env()` reads `SUPABASE_URL` and `SUPABASE_ANON_KEY`. `RealtimeConfig::from_env()` reads the same plus access-token bootstrap.

## Platform support
All platforms.

## Status
beta. Used by `copypaste-daemon` under the `cloud-sync` feature.

## Internal vs published
Internal workspace crate. Not published to crates.io.

## Quick example

```rust,no_run
use copypaste_supabase::auth::AuthClient;

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let client = AuthClient::from_env()?;
let session = client.sign_in("user@example.com", "s3cr3t").await?;
println!("access_token: {}", session.access_token);
# Ok(())
# }
```

## Tests
5 integration tests under `tests/`: auth, token refresh, realtime subscribe, reconnect backoff, RLS policies.

```bash
cargo test -p copypaste-supabase
```
