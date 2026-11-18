//! Supabase Storage requests for encrypted file objects.
//!
//! The deployment side is `supabase/migrations/*_clipboard_files_storage.sql`:
//! a private bucket whose policies confine an account to `{user id}/…`.
//! Bodies are already sealed (`crypto::file`); this module only moves bytes.

use std::time::Duration;

use reqwest::{header::CONTENT_TYPE, StatusCode};

use crate::rest::body::{read_capped, BodyError};
use crate::{crypto::file::OBJECT_PLAINTEXT_BYTES, CloudConfig, HttpClient};

pub const FILE_BUCKET: &str = "clipboard-files";
/// One sealed object: padded plaintext, nonce and tag.
pub const MAX_OBJECT_BYTES: usize = OBJECT_PLAINTEXT_BYTES + 64;
const OBJECT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("the access token was refused")]
    Unauthorized,
    #[error("the file is not in cloud storage")]
    NotFound,
    #[error("cloud storage refused the request (status {status})")]
    Refused { status: u16 },
    #[error("could not reach cloud storage")]
    Network,
    #[error("cloud storage returned an oversized object")]
    TooLarge,
}

/// The object path for one item's object `index`.
#[must_use]
pub fn object_path(user_id: &str, object_id: &str, index: u64) -> String {
    format!("{user_id}/{object_id}/{index:05}")
}

#[derive(Clone)]
pub struct SupabaseStorage {
    config: CloudConfig,
    http: reqwest::Client,
}

impl SupabaseStorage {
    #[must_use]
    pub fn with_client(config: CloudConfig, http: HttpClient) -> Self {
        Self {
            config,
            http: http.0,
        }
    }

    fn request(&self, method: reqwest::Method, path: &str, token: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, self.config.endpoint(path))
            .header("apikey", self.config.anon_key())
            .bearer_auth(token)
            .timeout(OBJECT_TIMEOUT)
    }

    /// Stores one sealed object. An object that already exists under the same
    /// name is the same item's seal, so it counts as uploaded.
    pub async fn upload(&self, token: &str, path: &str, body: Vec<u8>) -> Result<(), StorageError> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("storage/v1/object/{FILE_BUCKET}/{path}"),
                token,
            )
            .header(CONTENT_TYPE, "application/octet-stream")
            .header("x-upsert", "false")
            .body(body)
            .send()
            .await
            .map_err(|_| StorageError::Network)?;
        match response.status() {
            status if status.is_success() => Ok(()),
            StatusCode::CONFLICT => Ok(()),
            status => Err(classify(status)),
        }
    }

    pub async fn download(&self, token: &str, path: &str) -> Result<Vec<u8>, StorageError> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("storage/v1/object/authenticated/{FILE_BUCKET}/{path}"),
                token,
            )
            .send()
            .await
            .map_err(|_| StorageError::Network)?;
        let status = response.status();
        if !status.is_success() {
            return Err(match status {
                // Storage answers a missing object with 400 on some versions.
                StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST => StorageError::NotFound,
                status => classify(status),
            });
        }
        read_capped(response, MAX_OBJECT_BYTES)
            .await
            .map_err(|error| match error {
                BodyError::TooLarge => StorageError::TooLarge,
                BodyError::Network(_) => StorageError::Network,
            })
    }

    /// Removes objects; names that are already gone are not an error.
    pub async fn delete(&self, token: &str, paths: &[String]) -> Result<(), StorageError> {
        if paths.is_empty() {
            return Ok(());
        }
        let response = self
            .request(
                reqwest::Method::DELETE,
                &format!("storage/v1/object/{FILE_BUCKET}"),
                token,
            )
            .json(&serde_json::json!({ "prefixes": paths }))
            .send()
            .await
            .map_err(|_| StorageError::Network)?;
        match response.status() {
            status if status.is_success() => Ok(()),
            status => Err(classify(status)),
        }
    }
}

fn classify(status: StatusCode) -> StorageError {
    match status {
        StatusCode::UNAUTHORIZED => StorageError::Unauthorized,
        StatusCode::NOT_FOUND => StorageError::NotFound,
        status => StorageError::Refused {
            status: status.as_u16(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_bytes, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn objects_travel_under_the_account_folder_with_both_credentials() {
        let server = MockServer::start().await;
        let name = object_path("user-1", "0123456789abcdef0123456789abcdef", 1);
        Mock::given(method("POST"))
            .and(path(format!("/storage/v1/object/{FILE_BUCKET}/{name}")))
            .and(header("apikey", "anon"))
            .and(header("authorization", "Bearer token"))
            .and(header("x-upsert", "false"))
            .and(body_bytes(b"sealed".to_vec()))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/authenticated/{FILE_BUCKET}/{name}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"sealed".to_vec()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/authenticated/{FILE_BUCKET}/missing"
            )))
            .respond_with(ResponseTemplate::new(400))
            .mount(&server)
            .await;
        let storage = SupabaseStorage::with_client(
            CloudConfig::new_loopback(server.uri(), "anon").unwrap(),
            HttpClient::default(),
        );
        storage
            .upload("token", &name, b"sealed".to_vec())
            .await
            .unwrap();
        assert_eq!(storage.download("token", &name).await.unwrap(), b"sealed");
        assert!(matches!(
            storage.download("token", "missing").await,
            Err(StorageError::NotFound)
        ));
    }
}
