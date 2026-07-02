// ---------------------------------------------------------------------------
// src/lib/fixtures/device.ts — typed device fixture factories.
//
// DEV-only. See index.ts for the import-boundary rule.
// ---------------------------------------------------------------------------

import type { DiscoveredDevice, OwnDeviceInfo, PairedDevice } from "../ipc";
import { FIXTURE_OWN_FINGERPRINT } from "./ids";
import { days, hours } from "./time";

/** Default fingerprint for a fixture peer — distinct from the own-device one. */
const FIXTURE_PEER_FINGERPRINT =
  "bbccddee112233445566778899aabbccddeeff0011223344556677889900aabb";

/**
 * Typed factory for a paired peer ({@link PairedDevice}), with per-field
 * override support (design.md Decision 7/G3). Defaults describe a healthy,
 * SAS-verified, online macOS peer synced over P2P.
 *
 * The default `fingerprint` is a fixed constant — override it per call when
 * rendering more than one device at once so React keys stay stable.
 */
export function makeDevice(over: Partial<PairedDevice> = {}): PairedDevice {
  return {
    fingerprint: FIXTURE_PEER_FINGERPRINT,
    name: "Fixture MacBook Pro",
    added_at: Math.floor(days(90) / 1000),
    address: "192.168.1.42:7878",
    sync_key_b64: null,
    model: "MacBook Pro 16-inch (2023)",
    os_version: "macOS 15.5",
    app_version: "0.7.1",
    local_ip: "192.168.1.42",
    public_ip: "203.0.113.10",
    first_sync_at: Math.floor(days(89) / 1000),
    last_sync_at: Math.floor(hours(2) / 1000),
    online: true,
    last_seen_secs: 4,
    latency_ms: 12,
    trust: "verified",
    transport: "p2p",
    supabase_account_id: null,
    ...over,
  };
}

/**
 * Typed factory for an unpaired LAN device seen via mDNS-SD
 * ({@link DiscoveredDevice}), with per-field override support.
 */
export function makeDiscoveredDevice(over: Partial<DiscoveredDevice> = {}): DiscoveredDevice {
  return {
    device_id:
      "eeff005566778899aabbccddeeff001122334455667788990011aabb00eeff11",
    device_name: "Fixture LAN Device",
    ip_addrs: ["192.168.1.100"],
    port: 7878,
    bport: 7879,
    paired: false,
    ...over,
  };
}

/**
 * Typed factory for this device's own rich identity
 * ({@link OwnDeviceInfo}), with per-field override support.
 */
export function makeOwnDeviceInfo(over: Partial<OwnDeviceInfo> = {}): OwnDeviceInfo {
  return {
    fingerprint: FIXTURE_OWN_FINGERPRINT,
    device_name: "Fixture MacBook Air",
    device_model: "MacBook Air 15-inch (M3)",
    os_version: "macOS 15.5",
    app_version: "0.7.1",
    local_ip: "192.168.1.50",
    public_ip: "203.0.113.42",
    ...over,
  };
}
