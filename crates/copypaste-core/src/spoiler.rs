//! Temporary local presentation state shared by all clients of one runtime.

use std::collections::BTreeSet;
use std::sync::Mutex;

#[derive(Default)]
pub struct Spoilers(Mutex<(u64, BTreeSet<String>)>);

impl Spoilers {
    pub fn snapshot(&self) -> (u64, Vec<String>) {
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        (state.0, state.1.iter().cloned().collect())
    }

    /// `None` conceals every clip. No content or state is persisted or synced.
    pub fn set(&self, id: Option<&str>, revealed: bool) -> (u64, Vec<String>) {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        match id {
            Some(id) if revealed => {
                state.1.insert(id.to_owned());
            }
            Some(id) => {
                state.1.remove(id);
            }
            None => state.1.clear(),
        }
        state.0 += 1;
        (state.0, state.1.iter().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::Spoilers;

    #[test]
    fn snapshots_order_reveal_and_conceal_without_persisting() {
        let state = Spoilers::default();
        assert_eq!(state.snapshot(), (0, vec![]));
        assert_eq!(state.set(Some("clip"), true), (1, vec!["clip".into()]));
        assert_eq!(state.set(Some("other"), true).0, 2);
        assert_eq!(state.set(Some("clip"), false), (3, vec!["other".into()]));
        assert_eq!(state.set(None, false), (4, vec![]));
        assert_eq!(Spoilers::default().snapshot(), (0, vec![]));
    }
}
