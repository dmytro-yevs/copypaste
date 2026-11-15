//! `copypaste space`: members, invitations and removal across the space.
use copypaste_ipc::{Method, ResponseData, SpaceData};
use std::io::{IsTerminal, Write};

use crate::cli::SpaceAction;
use crate::error::CliError;
use crate::{client, cloud, now_ms, out, render};

async fn data(method: Method) -> Result<Option<ResponseData>, CliError> {
    client::into_data(client::request(method).await?)
}

fn expect_space(data: Option<ResponseData>) -> Result<SpaceData, CliError> {
    match data {
        Some(ResponseData::Space(space)) => Ok(space),
        _ => Err(CliError::local("the daemon did not answer with the space")),
    }
}

pub fn space_text(space: &SpaceData) -> String {
    let mut lines = Vec::new();
    if !space.member {
        lines.push("this device is not in a space; join one with `copypaste space join`".into());
        return lines.join("\n");
    }
    lines.push(match space.key_epoch {
        Some(epoch) => format!("cloud key: epoch {epoch}"),
        None => "cloud key: not set up (cloud sync is off)".into(),
    });
    if space.key_epoch.is_some() {
        lines.push(if space.passphrase_recovery {
            "passphrase recovery: on".into()
        } else {
            "passphrase recovery: off until a new sync passphrase is set".into()
        });
    }
    if !space.waiting_pairing_ids.is_empty() {
        lines.push(format!(
            "waiting for the new key: {}",
            space.waiting_pairing_ids.join(", ")
        ));
    }
    lines.join("\n")
}

fn confirm(question: &str, refusal: &str) -> Result<bool, CliError> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return Err(CliError::local(refusal.to_string()));
    }
    eprint!("{question} [y/N] ");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    stdin
        .read_line(&mut answer)
        .map_err(|e| CliError::local(format!("could not read the answer: {e}")))?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn json(value: &impl serde::Serialize) -> Result<(), CliError> {
    out(&serde_json::to_string_pretty(value)
        .map_err(|e| CliError::local(format!("could not render the reply as JSON: {e}")))?);
    Ok(())
}

pub async fn run(action: Option<&SpaceAction>, as_json: bool) -> Result<(), CliError> {
    match action.unwrap_or(&SpaceAction::Status) {
        SpaceAction::Status => {
            let space = expect_space(data(Method::Space).await?)?;
            let peers = client::expect_peers(data(Method::Peers).await?)?;
            if as_json {
                return json(&serde_json::json!({ "space": space, "members": peers }));
            }
            out(&space_text(&space));
            out(&render::peers_table(
                &peers,
                now_ms(),
                "no other devices in this space",
            ));
        }
        SpaceAction::Invite => {
            let invite = client::expect_pairing(data(Method::PairCreateInvite).await?)?;
            out(&render::pairing_text(&invite));
        }
        SpaceAction::Join { code, addr } => {
            let progress = client::expect_pairing_progress(
                data(Method::PairJoin {
                    code: code.clone(),
                    addr: addr.clone(),
                })
                .await?,
            )?;
            out(&render::pairing_progress_text(&progress));
        }
        SpaceAction::Confirm | SpaceAction::Reject => {
            let accept = matches!(action, Some(SpaceAction::Confirm));
            let progress =
                client::expect_pairing_progress(data(Method::PairConfirm { accept }).await?)?;
            out(&render::pairing_progress_text(&progress));
        }
        SpaceAction::Remove {
            pairing_id,
            new_passphrase,
            sign_out_others,
            yes,
        } => {
            if !yes
                && !confirm(
                    &format!(
                        "Remove {pairing_id} from the space on every device? It cannot read \
                         anything synced from now on."
                    ),
                    "refusing to remove a device without confirmation: pass --yes",
                )?
            {
                out("cancelled; that device is still in the space");
                return Ok(());
            }
            // Read before anything changes, so a missing secret aborts cleanly.
            let passphrase = new_passphrase
                .then(cloud::read_new_passphrase)
                .transpose()?;
            data(Method::Revoke {
                pairing_id: pairing_id.clone(),
            })
            .await?;
            out(&format!("removed {pairing_id} from the space"));
            if let Some(passphrase) = passphrase {
                data(Method::CloudChangePassphrase { passphrase }).await?;
                out("set a new sync passphrase for the new cloud key");
            }
            if *sign_out_others {
                data(Method::CloudSignOutOthers).await?;
                out("signed other devices out of the cloud account");
            }
        }
        SpaceAction::Leave { yes } => {
            if !yes
                && !confirm(
                    "Leave this space? Other devices remove this one, and joining again \
                     needs a new confirmed pairing.",
                    "refusing to leave without confirmation: pass --yes",
                )?
            {
                out("cancelled; this device is still in the space");
                return Ok(());
            }
            let space = expect_space(data(Method::SpaceLeave).await?)?;
            if as_json {
                return json(&space);
            }
            out("left the space");
        }
    }
    Ok(())
}
