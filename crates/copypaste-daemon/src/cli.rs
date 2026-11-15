//! The standalone daemon command line.

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

    /// Internal app-to-daemon lifetime contract. The app supplies a private
    /// stdin pipe and the daemon shuts down when that pipe reaches EOF.
    ///
    /// Hidden because a terminal-launched daemon must remain independent of
    /// its terminal; only the bundled macOS app supplies the pipe.
    #[arg(long, hide = true)]
    pub app_parent: bool,

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
    /// Stores a manual name that takes precedence over the system name.
    /// Without an override, the name follows the operating system.
    #[arg(long, value_name = "NAME")]
    pub device_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_daemon_does_not_require_an_app_parent() {
        let args = Args::try_parse_from(["copypaste-daemon"])
            .expect("the ordinary CLI invocation remains valid");
        assert!(!args.app_parent);
    }
}
