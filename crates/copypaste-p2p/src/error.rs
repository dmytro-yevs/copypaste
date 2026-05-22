use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("mDNS daemon error: {0}")]
    Daemon(String),

    #[error("Failed to register service: {0}")]
    Register(String),

    #[error("Failed to browse services: {0}")]
    Browse(String),

    #[error("Service already registered")]
    AlreadyRegistered,
}
