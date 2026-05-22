//! Supabase Realtime WebSocket client for CopyPaste clipboard sync.
//!
//! Implements the Phoenix Channel protocol over WebSocket manually —
//! there is no official Rust Supabase SDK with Realtime support.
//!
//! # Usage
//!
//! ```no_run
//! use copypaste_supabase::{RealtimeClient, RealtimeConfig, ChangeEvent};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = RealtimeConfig::from_env().unwrap();
//!     let (client, mut rx) = RealtimeClient::new(config);
//!     let handle = client.connect().await.unwrap();
//!
//!     while let Some(event) = rx.recv().await {
//!         println!("Got event: {:?}", event);
//!     }
//!     handle.shutdown().await;
//! }
//! ```

pub mod protocol;
pub mod realtime;

pub use protocol::{PhoenixMessage, PhoenixEvent, ChangeEvent, ChangeType};
pub use realtime::{RealtimeClient, RealtimeConfig, ClientHandle, RealtimeError};
