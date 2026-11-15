//! Space key envelopes: immutable rows, one per epoch, sender and recipient.
use super::{RestError, SupabaseRest};
use copypaste_sync::space::{SpaceEnvelope, RECOVERY_RECIPIENT};

pub const SPACE_TABLE: &str = "space_key_envelopes";
pub const SPACE_PAGE_SIZE: usize = 64;
pub const SPACE_INDEX_SIZE: usize = 512;
const COLUMNS: &str = "epoch,sender,recipient,nonce,ciphertext";

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceIndexEntry {
    pub sender: String,
    pub recipient: String,
}

fn valid_id(id: &str) -> Result<(), RestError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(RestError::Malformed);
    }
    Ok(())
}

impl SupabaseRest {
    /// Envelopes for this device and recovery envelopes, newest epoch first.
    pub async fn fetch_space_envelopes(
        &self,
        token: &str,
        device_id: &str,
    ) -> Result<Vec<SpaceEnvelope>, RestError> {
        valid_id(device_id)?;
        let url = self.config.endpoint(&format!("/rest/v1/{SPACE_TABLE}"));
        let query = [
            ("select", COLUMNS.to_owned()),
            (
                "recipient",
                format!("in.({device_id},{RECOVERY_RECIPIENT})"),
            ),
            ("order", "epoch.desc".to_owned()),
            ("limit", SPACE_PAGE_SIZE.to_string()),
        ];
        let body = self
            .send(token, || {
                self.http
                    .get(url.clone())
                    .query(&query)
                    .header("Accept", "application/json")
            })
            .await?;
        if body.len() > SPACE_PAGE_SIZE * (copypaste_sync::space::MAX_ENVELOPE_CIPHERTEXT + 512) {
            return Err(RestError::Malformed);
        }
        let rows: Vec<SpaceEnvelope> =
            serde_json::from_str(&body).map_err(|_| RestError::Malformed)?;
        if rows.len() > SPACE_PAGE_SIZE {
            return Err(RestError::Malformed);
        }
        Ok(rows
            .into_iter()
            .filter(|row| {
                row.is_valid()
                    && (row.recipient == device_id || row.recipient == RECOVERY_RECIPIENT)
            })
            .collect())
    }
    /// Who already has an envelope for `epoch`.
    pub async fn fetch_space_index(
        &self,
        token: &str,
        epoch: u64,
    ) -> Result<Vec<SpaceIndexEntry>, RestError> {
        let url = self.config.endpoint(&format!("/rest/v1/{SPACE_TABLE}"));
        let query = [
            ("select", "sender,recipient".to_owned()),
            ("epoch", format!("eq.{epoch}")),
            ("limit", SPACE_INDEX_SIZE.to_string()),
        ];
        let body = self
            .send(token, || {
                self.http
                    .get(url.clone())
                    .query(&query)
                    .header("Accept", "application/json")
            })
            .await?;
        if body.len() > SPACE_INDEX_SIZE * 512 {
            return Err(RestError::Malformed);
        }
        let rows: Vec<SpaceIndexEntry> =
            serde_json::from_str(&body).map_err(|_| RestError::Malformed)?;
        if rows.len() > SPACE_INDEX_SIZE {
            return Err(RestError::Malformed);
        }
        Ok(rows)
    }
    pub async fn insert_space_envelopes(
        &self,
        token: &str,
        rows: &[SpaceEnvelope],
    ) -> Result<(), RestError> {
        if rows.is_empty() || rows.len() > SPACE_PAGE_SIZE || rows.iter().any(|row| !row.is_valid())
        {
            return Err(RestError::InvalidItem {
                reason: "invalid space envelope",
            });
        }
        let url = self.config.endpoint(&format!("/rest/v1/{SPACE_TABLE}"));
        self.send(token, || {
            self.http
                .post(url.clone())
                .query(&[("on_conflict", "user_id,epoch,sender,recipient")])
                .header("Prefer", "resolution=ignore-duplicates,return=minimal")
                .json(rows)
        })
        .await?;
        Ok(())
    }
    /// Removes this device's envelopes of epochs before `epoch`.
    pub async fn delete_space_envelopes(
        &self,
        token: &str,
        sender: &str,
        before_epoch: u64,
    ) -> Result<(), RestError> {
        valid_id(sender)?;
        let url = self.config.endpoint(&format!("/rest/v1/{SPACE_TABLE}"));
        let query = [
            ("sender", format!("eq.{sender}")),
            ("epoch", format!("lt.{before_epoch}")),
        ];
        self.send(token, || {
            self.http
                .delete(url.clone())
                .query(&query)
                .header("Prefer", "return=minimal")
        })
        .await?;
        Ok(())
    }
}
