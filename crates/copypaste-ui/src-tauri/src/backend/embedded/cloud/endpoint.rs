use copypaste_cloud::credentials::CloudStateKey;
use copypaste_cloud::CloudConfig;
use copypaste_core::Store;

use super::validated_cloud_config;

pub(super) fn overlay(store: &Store, hosted: Option<CloudConfig>) -> Option<CloudConfig> {
    let url = store
        .state(CloudStateKey::EndpointUrl.as_str())
        .ok()
        .flatten();
    let key = store
        .state(CloudStateKey::EndpointAnonKey.as_str())
        .ok()
        .flatten();
    match (url, key) {
        (Some(url), Some(key)) => match validated_cloud_config(url, key) {
            Ok(config) => Some(config),
            Err(_) => {
                tracing::warn!("stored cloud endpoint was rejected; using the hosted default");
                hosted
            }
        },
        _ => hosted,
    }
}

pub(super) fn persist(
    store: &Store,
    url: &str,
    anon_key: &str,
) -> Result<(), copypaste_core::StoreError> {
    store.set_state_all(&[
        (CloudStateKey::EndpointUrl.as_str(), url),
        (CloudStateKey::EndpointAnonKey.as_str(), anon_key),
    ])
}

pub(super) fn clear(store: &Store) -> Result<(), copypaste_core::StoreError> {
    store.clear_state(&[
        CloudStateKey::EndpointUrl.as_str(),
        CloudStateKey::EndpointAnonKey.as_str(),
    ])
}
