#![allow(dead_code)]

mod daemon;
mod keychain;
mod clipboard;
mod ipc;
mod paths;
mod protocol;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let support_dir = paths::app_support_dir();
    std::fs::create_dir_all(&support_dir)?;

    daemon::run().await
}
