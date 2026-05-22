//! Supabase GoTrue authentication client for CopyPaste.
//!
//! # Quick start
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

pub mod auth;
pub mod error;
pub mod models;
pub mod store;

pub use auth::AuthClient;
pub use error::{AuthError, AuthResult};
pub use models::{Session, User};
pub use store::{InMemoryStore, SessionStore};
