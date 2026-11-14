//! The socket transport's own client-visible failure strings.
//!
//! Handler sentences, the failure classification and the two mappers are the
//! shared dispatch's, in `copypaste_runtime::messages`; what is here belongs to
//! framing, readiness and the watch slots, which only a daemon socket has.

pub(crate) use copypaste_runtime::messages::*;

pub(super) const MSG_NOT_READY: &str = "daemon is still starting up; retry shortly";
pub(super) const MSG_MALFORMED: &str = "malformed request";
pub(super) const MSG_TOO_LARGE: &str = "request exceeds the maximum frame size";
pub(super) const MSG_INTERNAL: &str = "the daemon failed to process the request";
pub(super) const MSG_WATCHERS_FULL: &str = "too many clients are already watching for changes";
pub(super) const MSG_ENDPOINTS_FULL: &str = "too many module connections are already open";
pub(super) const MSG_FORBIDDEN: &str = "only the CopyPaste app can perform this operation";
pub(super) const MSG_BAD_EVENT_FILTER: &str = "watch_events needs between 1 and 16 event kinds";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_messages_never_disclose_a_path_or_a_file() {
        for message in [
            MSG_NOT_READY,
            MSG_MALFORMED,
            MSG_TOO_LARGE,
            MSG_INTERNAL,
            MSG_WATCHERS_FULL,
            MSG_ENDPOINTS_FULL,
            MSG_FORBIDDEN,
            MSG_BAD_EVENT_FILTER,
        ] {
            assert!(
                !message.contains('/') && !message.contains('\\'),
                "{message}"
            );
            for fragment in [".db", ".sqlite", ".sock", "~"] {
                assert!(!message.contains(fragment), "{message}");
            }
        }
    }
}
