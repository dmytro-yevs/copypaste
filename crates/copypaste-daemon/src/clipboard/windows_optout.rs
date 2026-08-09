//! The Windows "do not record this" clipboard formats.
//!
//! macOS has the three `org.nspasteboard.*` markers of manifest 01 §3.4;
//! Windows has four format names that say the same thing, and honouring them is
//! the same rule (I-5). Recording a password manager's clipboard is the failure
//! this project takes most seriously.
//!
//! The *decision* lives here, apart from the backend that reads it, for the
//! reason [`super::change`] gives: what only one platform can run is what
//! regresses unnoticed. These functions compile and are tested on every host.

/// Formats whose mere presence forbids capture.
///
/// * `Clipboard Viewer Ignore` — the older convention, written by KeePass 2.x
///   and Sticky Password.
/// * `ExcludeClipboardContentFromMonitorProcessing` — Microsoft's own, and what
///   KeePassXC, 1Password and Chrome's incognito mode write.
///
/// Both are needed. Managers disagree about which they set — KeePassXC writes
/// the second and not the first — so a monitor that honours one of them still
/// records the other half of the field's passwords.
pub(super) const PRESENCE_MARKERS: [&str; 2] = [
    "Clipboard Viewer Ignore",
    "ExcludeClipboardContentFromMonitorProcessing",
];

/// Formats that carry a `DWORD`: zero forbids, non-zero permits.
///
/// Their names read as permissions rather than prohibitions, so presence alone
/// says nothing — the four bytes are the signal, which is why these two are
/// separated from the pair above.
pub(super) const FLAG_MARKERS: [&str; 2] =
    ["CanIncludeInClipboardHistory", "CanUploadToCloudClipboard"];

/// Decide one flag marker that is present on the clipboard.
///
/// `None` is a flag that is present but could not be read. Every unreadable or
/// malformed case answers "forbidden": a marker we cannot decide is one whose
/// writer went out of its way to say something, and AGENTS.md rule 4 fails
/// closed on exactly this shape of question.
pub(super) fn flag_forbids_capture(payload: Option<&[u8]>) -> bool {
    let Some(payload) = payload else {
        return true;
    };
    // `GlobalSize` may report the rounded-up allocation rather than the four
    // bytes that were written, so a longer payload is ordinary, not malformed.
    let Some(dword) = payload.first_chunk::<4>() else {
        return true;
    };
    u32::from_le_bytes(*dword) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_forbids_and_one_permits() {
        assert!(flag_forbids_capture(Some(&0u32.to_le_bytes())));
        assert!(!flag_forbids_capture(Some(&1u32.to_le_bytes())));
    }

    /// The allocation the writer made is not necessarily the one Windows
    /// reports, and a trailing byte must not turn a "no" into a "yes".
    #[test]
    fn a_padded_payload_is_read_from_its_first_four_bytes() {
        assert!(flag_forbids_capture(Some(&[0, 0, 0, 0, 0, 0, 0, 0])));
        assert!(!flag_forbids_capture(Some(&[1, 0, 0, 0, 0, 0, 0, 0])));
    }

    #[test]
    fn an_unreadable_or_truncated_flag_forbids_capture() {
        assert!(flag_forbids_capture(None));
        assert!(flag_forbids_capture(Some(&[])));
        assert!(flag_forbids_capture(Some(&[0, 0, 0])));
        assert!(flag_forbids_capture(Some(&[1, 0, 0])));
    }

    /// A duplicated entry would silently halve the set that is probed.
    #[test]
    fn every_marker_name_is_distinct_and_non_empty() {
        let mut names: Vec<&str> = PRESENCE_MARKERS
            .iter()
            .chain(&FLAG_MARKERS)
            .copied()
            .collect();
        assert!(names.iter().all(|name| !name.is_empty()));
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }
}
