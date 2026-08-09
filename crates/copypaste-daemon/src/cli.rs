//! The command line, and the cloud deployment it resolves.

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "copypaste-daemon",
    version,
    about = "CopyPaste clipboard daemon"
)]
pub struct Args {
    /// Directory holding the database and the IPC socket.
    ///
    /// Defaults to the platform application-data directory resolved by
    /// `copypaste_ipc`. Overriding it runs an instance that is fully isolated
    /// from the user's real history — that is what the tests and `--data-dir`
    /// demos rely on.
    #[arg(long, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    /// Stay attached to the terminal.
    ///
    /// The daemon never forks: backgrounding is the service manager's job
    /// (launchd on macOS). The flag exists so a service definition can state
    /// its intent, and it suppresses the notice printed when it is absent.
    #[arg(long)]
    pub foreground: bool,

    /// TCP port the peer listener binds.
    ///
    /// Fixed by default so an explicit address is short to type. Overriding it
    /// is what lets two daemons run on one host, which is how the peer-sync
    /// demo works; the pairing this daemon mints reports whichever port is in
    /// use, so the other device does not have to be told separately.
    #[arg(long, default_value_t = copypaste_p2p::DEFAULT_PORT)]
    pub port: u16,

    /// What peers call this device.
    ///
    /// Cosmetic and peer-visible. Stored on first run and kept afterwards, so
    /// passing it once is enough and a hostname change does not rename the
    /// device on every peer.
    #[arg(long, value_name = "NAME")]
    pub device_name: Option<String>,

    /// Supabase project URL for cloud sync, e.g. `https://abc.supabase.co`.
    ///
    /// Falls back to `COPYPASTE_CLOUD_URL`. Without both this and the anon key
    /// the daemon runs with cloud sync unconfigured, which is a supported
    /// state: peer sync and local history do not depend on it.
    #[arg(long, value_name = "URL")]
    pub cloud_url: Option<String>,

    /// Supabase publishable anon key. Falls back to `COPYPASTE_CLOUD_ANON_KEY`.
    ///
    /// Not a secret in the usual sense — row-level security is what restricts
    /// access — so it is ordinary configuration rather than a credential.
    #[arg(long, value_name = "KEY")]
    pub cloud_anon_key: Option<String>,
}

/// Resolve the deployment from flags, then the environment.
///
/// Both halves are required: a URL with no key cannot authenticate and a key
/// with no URL has nothing to talk to, so a half-configuration is reported as
/// unconfigured rather than failing at the first request.
pub fn cloud_config(
    args: &Args,
) -> Result<Option<copypaste_cloud::CloudConfig>, copypaste_cloud::CloudConfigError> {
    fn resolve(flag: Option<&String>, var: &str) -> Option<String> {
        flag.cloned()
            .or_else(|| std::env::var(var).ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }
    let Some(url) = resolve(args.cloud_url.as_ref(), "COPYPASTE_CLOUD_URL") else {
        return Ok(None);
    };
    let Some(anon_key) = resolve(args.cloud_anon_key.as_ref(), "COPYPASTE_CLOUD_ANON_KEY") else {
        return Ok(None);
    };
    copypaste_cloud::CloudConfig::new(url, anon_key).map(Some)
}
