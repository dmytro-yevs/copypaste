use super::semantics::PairingMessageId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct PairingCopy {
    pub title: &'static str,
    pub detail: &'static str,
}

pub(crate) fn copy(message_id: PairingMessageId) -> PairingCopy {
    match message_id {
        PairingMessageId::Ready => PairingCopy {
            title: "Pair a device",
            detail: "No device pairing is in progress.",
        },
        PairingMessageId::WaitingForPeer => PairingCopy {
            title: "Waiting for a device",
            detail: "Waiting for the other device to join.",
        },
        PairingMessageId::SecuringConnection => PairingCopy {
            title: "Securing the connection",
            detail: "Keep both devices nearby while CopyPaste establishes a secure connection.",
        },
        PairingMessageId::CompareCodes => PairingCopy {
            title: "Compare security codes",
            detail: "Confirm the code in the native security prompt.",
        },
        PairingMessageId::Paired => PairingCopy {
            title: "Paired",
            detail: "The device was paired successfully.",
        },
        PairingMessageId::Rejected => PairingCopy {
            title: "Pairing rejected",
            detail: "The security codes did not match, so no pairing was saved.",
        },
        PairingMessageId::Cancelled => PairingCopy {
            title: "Pairing cancelled",
            detail: "No pairing was saved.",
        },
        PairingMessageId::TimedOut => PairingCopy {
            title: "Pairing timed out",
            detail: "Check that both devices are on the same network and try again.",
        },
        PairingMessageId::CodeMismatch => PairingCopy {
            title: "Pairing didn't match",
            detail: "The security codes did not match. No device was paired.",
        },
        PairingMessageId::IncompatibleVersion => PairingCopy {
            title: "Pairing couldn't finish",
            detail: "The other device uses an incompatible version. No device was paired.",
        },
        PairingMessageId::Unreachable => PairingCopy {
            title: "Pairing couldn't finish",
            detail:
                "Could not reach the other device. Check that both devices are on the same network.",
        },
        PairingMessageId::Busy => PairingCopy {
            title: "Pairing is busy",
            detail: "Another pairing is already in progress.",
        },
        PairingMessageId::Limit => PairingCopy {
            title: "Pairing limit reached",
            detail: "Remove or revoke a paired device before trying again.",
        },
        PairingMessageId::Failed => PairingCopy {
            title: "Pairing failed",
            detail: "Pairing failed. No device was paired.",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_id_has_exact_path_and_secret_free_native_copy() {
        let expected = [
            (PairingMessageId::Ready, "Pair a device", "No device pairing is in progress."),
            (PairingMessageId::WaitingForPeer, "Waiting for a device", "Waiting for the other device to join."),
            (PairingMessageId::SecuringConnection, "Securing the connection", "Keep both devices nearby while CopyPaste establishes a secure connection."),
            (PairingMessageId::CompareCodes, "Compare security codes", "Confirm the code in the native security prompt."),
            (PairingMessageId::Paired, "Paired", "The device was paired successfully."),
            (PairingMessageId::Rejected, "Pairing rejected", "The security codes did not match, so no pairing was saved."),
            (PairingMessageId::Cancelled, "Pairing cancelled", "No pairing was saved."),
            (PairingMessageId::TimedOut, "Pairing timed out", "Check that both devices are on the same network and try again."),
            (PairingMessageId::CodeMismatch, "Pairing didn't match", "The security codes did not match. No device was paired."),
            (PairingMessageId::IncompatibleVersion, "Pairing couldn't finish", "The other device uses an incompatible version. No device was paired."),
            (PairingMessageId::Unreachable, "Pairing couldn't finish", "Could not reach the other device. Check that both devices are on the same network."),
            (PairingMessageId::Busy, "Pairing is busy", "Another pairing is already in progress."),
            (PairingMessageId::Limit, "Pairing limit reached", "Remove or revoke a paired device before trying again."),
            (PairingMessageId::Failed, "Pairing failed", "Pairing failed. No device was paired."),
        ];

        for (message_id, title, detail) in expected {
            assert_eq!(copy(message_id), PairingCopy { title, detail });
            let visible = format!("{title} {detail}").to_ascii_lowercase();
            for forbidden in ["/", "\\\\", "123456", "secret", "token"] {
                assert!(!visible.contains(forbidden), "{visible}");
            }
        }
    }
}
