//! Queues the platform-native copy acknowledgement sound.

/// Queue one acknowledgement without blocking the caller.
///
/// Failure is deliberately non-fatal: the operation being acknowledged has
/// already succeeded, and an unavailable audio device must not reverse it.
pub fn play() -> bool {
    platform::play()
}

#[cfg(target_os = "macos")]
mod platform {
    use std::process::{Command, Stdio};

    const PLAYER: &str = "/usr/bin/afplay";
    const SOUND: &str = "/System/Library/Sounds/Pop.aiff";

    pub fn play() -> bool {
        play_with(|player, sound| {
            Command::new(player)
                .arg(sound)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        })
    }

    fn play_with(spawn: impl FnOnce(&str, &str) -> std::io::Result<std::process::Child>) -> bool {
        match spawn(PLAYER, SOUND) {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    if let Err(error) = child.wait() {
                        tracing::debug!(%error, "could not reap the copy feedback player");
                    }
                });
                true
            }
            Err(error) => {
                tracing::debug!(%error, "could not play copy feedback");
                false
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn macos_uses_the_stock_copy_feedback_sound() {
            let mut requested = None;
            let _ = play_with(|player, sound| {
                requested = Some((player.to_owned(), sound.to_owned()));
                Err(std::io::Error::other("boundary assertion"))
            });
            assert_eq!(requested, Some((PLAYER.to_owned(), SOUND.to_owned())));
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use winsafe::{co, MessageBeep, SysResult};

    pub fn play() -> bool {
        play_with(MessageBeep)
    }

    fn play_with(call: impl FnOnce(co::MBP) -> SysResult<()>) -> bool {
        match call(co::MBP::OK) {
            Ok(()) => true,
            Err(error) => {
                tracing::debug!(%error, "could not play copy feedback");
                false
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn windows_queues_the_users_default_system_sound() {
            let mut requested = None;
            assert!(play_with(|sound| {
                requested = Some(sound);
                Ok(())
            }));
            assert_eq!(requested, Some(co::MBP::OK));
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    pub fn play() -> bool {
        false
    }
}
