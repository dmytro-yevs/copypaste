//! Local-only adapter for the browser development preview.

mod contract;
mod dispatch;
mod server;

pub use server::run_from_env;
