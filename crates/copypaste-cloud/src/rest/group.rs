//! Bounded encrypted membership rows, separate from ephemeral clipboard rows.
use super::{RestError, SupabaseRest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const GROUP_PAGE_SIZE: usize = 8;
pub const GROUP_CATALOG_PAGE_SIZE: usize = 64;
pub const MAX_GROUP_BYTES: usize = 768 * 1024;
pub const MAX_GROUP_ROWS: usize = 4096;
pub const GROUP_TABLE: &str = "device_group_states";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupRow {
    pub device_id: String,
    pub ciphertext: String,
    pub nonce: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupVersion {
    pub device_id: String,
    /// Computed by the database from nonce and ciphertext on every write.
    /// Account clients cannot preserve this token while changing the row.
    pub ciphertext_digest: String,
}
impl GroupVersion {
    pub fn validate(&self) -> Result<(), RestError> {
        validate_id(&self.device_id)?;
        if self.ciphertext_digest.len() != 64
            || !self
                .ciphertext_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(RestError::Malformed);
        }
        Ok(())
    }
}
impl GroupRow {
    pub(crate) fn ciphertext_digest(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(self.nonce.as_bytes());
        hash.update(self.ciphertext.as_bytes());
        hex::encode(hash.finalize())
    }
    pub fn aad_id(&self) -> String {
        format!("copypaste-group-{}", self.device_id)
    }
    pub fn validate(&self) -> Result<(), RestError> {
        validate_id(&self.device_id)?;
        if self.ciphertext.is_empty()
            || self.ciphertext.len() > (MAX_GROUP_BYTES + 16).div_ceil(3) * 4
            || self.nonce.len() != 32
        {
            return Err(RestError::Malformed);
        }
        Ok(())
    }
}
fn validate_id(id: &str) -> Result<(), RestError> {
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
    pub async fn fetch_group_catalog(
        &self,
        token: &str,
        after_device: Option<&str>,
    ) -> Result<Vec<GroupVersion>, RestError> {
        let url = self.config.endpoint(&format!("/rest/v1/{GROUP_TABLE}"));
        let mut query = vec![
            ("select", "device_id,ciphertext_digest".to_owned()),
            ("order", "device_id.asc".to_owned()),
            ("limit", GROUP_CATALOG_PAGE_SIZE.to_string()),
        ];
        if let Some(after) = after_device {
            validate_id(after)?;
            query.push(("device_id", format!("gt.{after}")));
        }
        let body = self
            .send(token, || {
                self.http
                    .get(url.clone())
                    .query(&query)
                    .header("Accept", "application/json")
            })
            .await?;
        // The transport page itself is bounded before parsing individual rows.
        if body.len() > GROUP_CATALOG_PAGE_SIZE * 512 {
            return Err(RestError::Malformed);
        }
        let rows: Vec<GroupVersion> =
            serde_json::from_str(&body).map_err(|_| RestError::Malformed)?;
        if rows.len() > GROUP_CATALOG_PAGE_SIZE {
            return Err(RestError::Malformed);
        }
        Ok(rows)
    }
    pub async fn fetch_groups(
        &self,
        token: &str,
        device_ids: &[String],
    ) -> Result<Vec<GroupRow>, RestError> {
        if device_ids.is_empty() || device_ids.len() > GROUP_PAGE_SIZE {
            return Err(RestError::Malformed);
        }
        for id in device_ids {
            validate_id(id)?;
        }
        let url = self.config.endpoint(&format!("/rest/v1/{GROUP_TABLE}"));
        let query = [
            ("select", "device_id,ciphertext,nonce".to_owned()),
            ("device_id", format!("in.({})", device_ids.join(","))),
            ("limit", GROUP_PAGE_SIZE.to_string()),
        ];
        let body = self
            .send(token, || {
                self.http
                    .get(url.clone())
                    .query(&query)
                    .header("Accept", "application/json")
            })
            .await?;
        if body.len() > GROUP_PAGE_SIZE * (MAX_GROUP_BYTES * 2 + 512) {
            return Err(RestError::Malformed);
        }
        let rows: Vec<GroupRow> = serde_json::from_str(&body).map_err(|_| RestError::Malformed)?;
        if rows.len() > GROUP_PAGE_SIZE
            || rows.iter().any(|row| !device_ids.contains(&row.device_id))
        {
            return Err(RestError::Malformed);
        }
        Ok(rows)
    }
    pub async fn upsert_group(&self, token: &str, row: &GroupRow) -> Result<(), RestError> {
        row.validate()?;
        let url = self.config.endpoint(&format!("/rest/v1/{GROUP_TABLE}"));
        self.send(token, || {
            self.http
                .post(url.clone())
                .query(&[("on_conflict", "user_id,device_id")])
                .header("Prefer", "resolution=merge-duplicates,return=minimal")
                .json(row)
        })
        .await?;
        Ok(())
    }
}
