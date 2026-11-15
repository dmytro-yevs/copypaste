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
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

    const SOUND: &[u8] = include_bytes!("../assets/Pop.wav");
    const FLAGS: u32 = SND_MEMORY | SND_ASYNC | SND_NODEFAULT;

    pub fn play() -> bool {
        play_with(|sound, flags| {
            // PlaySound retains the memory for asynchronous playback. SOUND
            // is embedded read-only data with process lifetime.
            unsafe { PlaySoundW(sound.as_ptr().cast(), std::ptr::null_mut(), flags) != 0 }
        })
    }

    fn play_with(call: impl FnOnce(&'static [u8], u32) -> bool) -> bool {
        call(SOUND, FLAGS)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn windows_queues_embedded_pop_without_a_system_sound_fallback() {
            assert!(play_with(|sound, flags| {
                assert_eq!(&sound[..4], b"RIFF");
                assert_eq!(&sound[8..12], b"WAVE");
                assert_eq!(flags, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
                true
            }));
        }

        #[test]
        fn unavailable_audio_is_non_fatal_and_has_no_beep_fallback() {
            assert!(!play_with(|_, _| false));
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    pub fn play() -> bool {
        false
    }
}
