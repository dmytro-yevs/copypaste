//! Supabase client for CopyPaste: GoTrue authentication + Realtime WebSocket sync.
//!
//! Implements the Phoenix Channel protocol over WebSocket manually —
//! there is no official Rust Supabase SDK with Realtime support.
//!
//! # Auth quick start
//!
//! ```no_run
//! use copypaste_supabase::auth::AuthClient;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Reads SUPABASE_URL and SUPABASE_ANON_KEY from environment.
//!     let client = AuthClient::from_env().unwrap();
//!     let session = client.sign_in("user@example.com", "s3cr3t").await.unwrap();
//!     println!("token: {}", session.access_token);
//! }
//! ```
//!
//! # Realtime quick start
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

pub mod auth;
pub mod error;
pub mod models;
pub mod protocol;
pub mod realtime;
pub mod store;

// Auth re-exports
pub use auth::AuthClient;
pub use error::{AuthError, AuthResult};
pub use models::{Session, User};
pub use store::{InMemoryStore, SessionStore};

// Realtime re-exports
pub use protocol::{ChangeEvent, ChangeType, PhoenixEvent, PhoenixMessage};
pub use realtime::{ClientHandle, RealtimeClient, RealtimeConfig, RealtimeError};
