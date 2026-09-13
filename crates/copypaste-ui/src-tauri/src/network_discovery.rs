use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::OnceLock;

use copypaste_core::p2p_contract;
use copypaste_ipc::DiscoveredDevice;
use copypaste_p2p::discovery::DiscoveredPeer;
use copypaste_p2p::{DeviceClass, DevicePlatform, DeviceProfile};
use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager as _, Wry};

const PLUGIN_PACKAGE: &str = "com.copypaste.app";
const PLUGIN_CLASS: &str = "NetworkDiscoveryPlugin";
const PORT: u16 = copypaste_p2p::DEFAULT_PORT;

static DISCOVERY: OnceLock<AndroidNetworkDiscovery> = OnceLock::new();

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
        let mut attributes = HashMap::new();
        attributes.insert("v".into(), "1".into());
        attributes.insert("n".into(), name.to_string());
        attributes.insert("pf".into(), "android".into());
        attributes.insert("dc".into(), "phone".into());
        for (index, id) in pairing_ids.iter().take(16).enumerate() {
            attributes.insert(format!("p{index}"), id.clone());
        }
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

    pub async fn resolved(&self) -> Vec<DiscoveredDevice> {
        let peers = self
            .0
            .run_mobile_plugin_async::<ResolvedPeers>("resolved", ())
            .await
            .map(|result| result.peers)
            .unwrap_or_default();
        let now = copypaste_core::now_ms();
        peers
            .into_iter()
            .filter_map(|peer| nsd_device(peer, now))
            .collect()
    }
}

fn nsd_device(peer: ResolvedPeer, now_ms: i64) -> Option<DiscoveredDevice> {
    if peer.port == 0 {
        return None;
    }
    let host: std::net::IpAddr = peer.host.parse().ok()?;
    let addr = SocketAddr::new(host, peer.port);
    let name = peer
        .attributes
        .get("n")
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| peer.service_name.clone());
    let pairing_ids = (0..16)
        .filter_map(|index| peer.attributes.get(&format!("p{index}")).cloned())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    let found = DiscoveredPeer {
        discovery_id: format!("nsd:{}", peer.service_name),
        pairing_ids,
        name,
        profile: Some(DeviceProfile {
            app_version: peer.attributes.get("av").cloned(),
            protocol_version: peer
                .attributes
                .get("pv")
                .and_then(|value| value.parse().ok()),
            platform: match peer.attributes.get("pf").map(String::as_str) {
                Some("macos") => DevicePlatform::Macos,
                Some("windows") => DevicePlatform::Windows,
                Some("android") => DevicePlatform::Android,
                _ => DevicePlatform::Unknown,
            },
            device_class: match peer.attributes.get("dc").map(String::as_str) {
                Some("desktop") => DeviceClass::Desktop,
                Some("laptop") => DeviceClass::Laptop,
                Some("phone") => DeviceClass::Phone,
                Some("tablet") => DeviceClass::Tablet,
                _ => DeviceClass::Unknown,
            },
            os_name: peer.attributes.get("os").cloned(),
            os_version: peer.attributes.get("ov").cloned(),
            model: peer.attributes.get("m").cloned(),
        }),
        addr,
        last_seen_ms: now_ms,
    };
    Some(p2p_contract::discovered_device(found, false))
}

pub async fn enrich_discovered(
    name: &str,
    pairing_ids: &[String],
    devices: Vec<DiscoveredDevice>,
) -> Vec<DiscoveredDevice> {
    let Some(discovery) = DISCOVERY.get() else {
        return devices;
    };
    let _ = discovery.acquire().await;
    let advertise_name = if name.is_empty() { "CopyPaste" } else { name };
    let _ = discovery.advertise(advertise_name, pairing_ids).await;
    merge_discovered(devices, discovery.resolved().await)
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
