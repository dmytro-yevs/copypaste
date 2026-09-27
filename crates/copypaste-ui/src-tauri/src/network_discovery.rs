use std::collections::HashMap;
use std::sync::OnceLock;

use copypaste_core::p2p_contract;
use copypaste_ipc::DiscoveredDevice;
use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager as _, Wry};

use crate::backend::{BackendError, Result};
use crate::network_discovery_lifecycle::{
    Desired, LifecycleCoordinator, LifecycleFuture, LifecycleOperation,
};

const PLUGIN_PACKAGE: &str = "com.copypaste.app";
const PLUGIN_CLASS: &str = "NetworkDiscoveryPlugin";
const PORT: u16 = copypaste_p2p::DEFAULT_PORT;
const MSG_DISCOVERY_UNAVAILABLE: &str = "Network discovery is unavailable.";

static DISCOVERY: OnceLock<AndroidNetworkDiscovery> = OnceLock::new();
static LIFECYCLE: OnceLock<LifecycleCoordinator> = OnceLock::new();
static LOCAL_ADDRS: OnceLock<copypaste_p2p::netif::LocalAddrs> = OnceLock::new();

#[derive(Deserialize)]
struct Availability {
    available: bool,
}

#[derive(Deserialize)]
struct ResolvedPeers {
    peers: Vec<ResolvedPeer>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedPeer {
    service_name: String,
    host: String,
    port: u16,
    #[serde(default)]
    attributes: HashMap<String, String>,
    last_seen_ms: i64,
}

#[derive(Serialize)]
struct AdvertiseArgs<'a> {
    name: &'a str,
    port: u16,
    attributes: HashMap<String, String>,
}

/// Android drops multicast packets unless an app holds this system-managed
/// Wi-Fi lock. Discovery keeps working on non-Wi-Fi networks without it.
/// [NsdManager] browse/register is the platform DNS-SD path used alongside it.
#[derive(Clone)]
pub struct AndroidNetworkDiscovery(PluginHandle<Wry>);

impl AndroidNetworkDiscovery {
    pub async fn acquire(&self) -> bool {
        self.0
            .run_mobile_plugin_async::<Availability>("acquire", ())
            .await
            .map(|result| result.available)
            .unwrap_or(false)
    }

    pub async fn advertise(&self, name: &str, pairing_ids: &[String]) -> bool {
        let attributes = match copypaste_p2p::discovery::advertisement_attributes(name, pairing_ids)
        {
            Ok(attributes) => attributes,
            Err(error) => {
                tracing::warn!(%error, "Android NSD advertisement is invalid");
                return false;
            }
        };
        self.0
            .run_mobile_plugin_async::<Availability>(
                "advertise",
                AdvertiseArgs {
                    name,
                    port: PORT,
                    attributes,
                },
            )
            .await
            .map(|result| result.available)
            .unwrap_or(false)
    }

    pub async fn resolved(&self, known_pairing_ids: &[String]) -> Result<Vec<DiscoveredDevice>> {
        let peers = self
            .0
            .run_mobile_plugin_async::<ResolvedPeers>("resolved", ())
            .await
            .map(|result| result.peers)
            .map_err(|_| BackendError::Unsupported(MSG_DISCOVERY_UNAVAILABLE))?;
        let now = copypaste_core::now_ms();
        Ok(peers
            .into_iter()
            .filter_map(|peer| nsd_device(peer, now, known_pairing_ids))
            .take(copypaste_p2p::discovery::MAX_PEERS)
            .collect())
    }
}

struct AndroidLifecycleOperation(AndroidNetworkDiscovery);

impl LifecycleOperation for AndroidLifecycleOperation {
    fn acquire(&self) -> LifecycleFuture<'_, bool> {
        Box::pin(self.0.acquire())
    }

    fn advertise<'a>(
        &'a self,
        name: &'a str,
        pairing_ids: &'a [String],
    ) -> LifecycleFuture<'a, bool> {
        Box::pin(self.0.advertise(name, pairing_ids))
    }

    fn release(&self) -> LifecycleFuture<'_, ()> {
        Box::pin(async move {
            let _ = self
                .0
                 .0
                .run_mobile_plugin_async::<serde_json::Value>("release", ())
                .await;
        })
    }
}

pub fn reconcile(name: String, pairing_ids: Vec<String>, visible: bool) {
    let lifecycle = LIFECYCLE.get_or_init(LifecycleCoordinator::default);
    if !lifecycle.reconcile(Desired {
        name,
        pairing_ids,
        visible,
    }) {
        return;
    }
    tauri::async_runtime::spawn(drain_lifecycle());
}

async fn drain_lifecycle() {
    let lifecycle = LIFECYCLE
        .get()
        .expect("reconcile starts the lifecycle before draining it");
    let Some(discovery) = DISCOVERY.get().cloned() else {
        lifecycle.stop();
        return;
    };
    lifecycle.drain(&AndroidLifecycleOperation(discovery)).await;
}

fn nsd_device(
    peer: ResolvedPeer,
    now_ms: i64,
    known_pairing_ids: &[String],
) -> Option<DiscoveredDevice> {
    if peer.port == 0 {
        return None;
    }
    let host: std::net::IpAddr = peer.host.parse().ok()?;
    if copypaste_p2p::netif::is_own_endpoint(
        LOCAL_ADDRS.get_or_init(copypaste_p2p::netif::LocalAddrs::default),
        PORT,
        std::net::SocketAddr::new(host, peer.port),
        now_ms,
    ) {
        return None;
    }
    let found = copypaste_p2p::discovery::peer_from_record(
        &peer.service_name,
        host,
        peer.port,
        &peer.attributes,
        peer.last_seen_ms,
    )?;
    let paired = found
        .pairing_ids
        .iter()
        .any(|id| known_pairing_ids.contains(id));
    copypaste_p2p::discovery::is_peer_fresh(found.last_seen_ms, now_ms)
        .then(|| p2p_contract::discovered_device(found, paired))
}

pub async fn enrich_discovered(
    name: &str,
    pairing_ids: &[String],
    devices: Vec<DiscoveredDevice>,
) -> Result<Vec<DiscoveredDevice>> {
    let Some(discovery) = DISCOVERY.get() else {
        return Err(BackendError::Unsupported(MSG_DISCOVERY_UNAVAILABLE));
    };
    let _ = name;
    match discovery.resolved(pairing_ids).await {
        Ok(extra) => Ok(merge_discovered(devices, extra)),
        Err(error) if devices.is_empty() => Err(error),
        Err(error) => {
            tracing::warn!(%error, "NSD resolved peers could not be read");
            Ok(devices)
        }
    }
}

pub fn merge_discovered(
    mut devices: Vec<DiscoveredDevice>,
    extra: Vec<DiscoveredDevice>,
) -> Vec<DiscoveredDevice> {
    for device in extra {
        if devices.iter().any(|existing| {
            existing.discovery_id == device.discovery_id || existing.addr == device.addr
        }) {
            continue;
        }
        devices.push(device);
    }
    devices
}

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("android-network-discovery")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS)?;
            let discovery = AndroidNetworkDiscovery(handle);
            let _ = DISCOVERY.set(discovery.clone());
            app.manage(discovery);
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved_peer(host: &str, port: u16, last_seen_ms: i64) -> ResolvedPeer {
        ResolvedPeer {
            service_name: "phone._copypaste._tcp.local.".into(),
            host: host.into(),
            port,
            attributes: copypaste_p2p::discovery::advertisement_attributes("Phone", &[]).unwrap(),
            last_seen_ms,
        }
    }

    #[test]
    fn nsd_records_filter_this_device_before_they_reach_discovery() {
        let now = copypaste_core::now_ms();

        assert!(nsd_device(resolved_peer("127.0.0.1", PORT, now), now, &[]).is_none());
        assert!(nsd_device(resolved_peer("::1", PORT, now), now, &[]).is_none());
        assert!(nsd_device(resolved_peer("::ffff:127.0.0.1", PORT, now), now, &[]).is_none());

        assert!(nsd_device(resolved_peer("127.0.0.1", PORT + 1, now), now, &[]).is_some());
        assert!(nsd_device(resolved_peer("192.0.2.1", PORT, now), now, &[]).is_some());
        assert!(nsd_device(resolved_peer("2001:db8::1", PORT, now), now, &[]).is_some());
    }

    #[test]
    fn merge_keeps_mdns_and_adds_unseen_nsd_peers() {
        let mdns = DiscoveredDevice {
            discovery_id: "mdns-1".into(),
            name: "Laptop".into(),
            addr: "192.0.2.1:47654".into(),
            last_seen_ms: 1,
            paired: false,
            details: None,
        };
        let nsd = DiscoveredDevice {
            discovery_id: "nsd:phone".into(),
            name: "Phone".into(),
            addr: "192.0.2.8:47654".into(),
            last_seen_ms: 2,
            paired: false,
            details: None,
        };
        let duplicate = DiscoveredDevice {
            discovery_id: "nsd-dup".into(),
            name: "Laptop Wi-Fi".into(),
            addr: "192.0.2.1:47654".into(),
            last_seen_ms: 3,
            paired: false,
            details: None,
        };
        let merged = merge_discovered(vec![mdns.clone()], vec![nsd.clone(), duplicate]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].discovery_id, "mdns-1");
        assert_eq!(merged[1].discovery_id, "nsd:phone");
    }
}
