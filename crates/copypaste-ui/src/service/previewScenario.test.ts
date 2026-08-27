import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { PairingCeremony } from "@/lib/ipc";
import { createPreviewInterceptor } from "@/service/previewIpc";
import {
  createPreviewScenarioStore,
  type PreviewDevice,
  type PreviewPairingPhase,
} from "@/service/previewScenario";

const PHONE: PreviewDevice = {
  id: "phone-1",
  name: "Nearby phone",
  type: "phone",
  platform: "android",
  osName: "Android",
  osVersion: "15",
  model: "Pixel 9",
  appVersion: "2.0.0-preview",
  protocolVersion: 2,
  latencyMs: 28,
  lanIp: "192.168.1.24",
  port: 49_200,
  publicIp: null,
  location: null,
  lastSeenAgeMs: 2_000,
  online: true,
  paired: false,
};

const WINDOWS_PC: PreviewDevice = {
  id: "windows-1",
  name: "Studio PC",
  type: "desktop",
  platform: "windows",
  osName: "Windows",
  osVersion: "11 24H2",
  model: "Surface Studio 2+",
  appVersion: "2.0.0-alpha.32",
  protocolVersion: 2,
  latencyMs: 31,
  lanIp: "192.168.1.31",
  port: 49_232,
  publicIp: "203.0.113.31",
  location: "Kyiv, Ukraine",
  lastSeenAgeMs: 4_000,
  online: true,
  paired: false,
};

beforeEach(() => {
  window.sessionStorage.clear();
  vi.stubGlobal("__COPYPASTE_APP_VERSION__", "2.0.0-preview");
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("preview scenario service", () => {
  it("persists device edits for the session and clears them on Reset to Live", () => {
    const store = createPreviewScenarioStore(window.sessionStorage);
    store.getState().addDevice(PHONE);
    store.getState().updateDevice(PHONE.id, {
      name: "Updated phone",
      type: "tablet",
      latencyMs: 44,
      lanIp: "192.168.1.25",
      osVersion: "16",
      model: "Pixel Tablet",
      appVersion: "2.1.0-preview",
      protocolVersion: 3,
      publicIp: "203.0.113.24",
      location: "Berlin, Germany",
    });
    store.getState().startPairing(PHONE.id);

    const restored = createPreviewScenarioStore(window.sessionStorage);
    expect(restored.getState().scenario).toMatchObject({
      pairing: "invite",
      pairingDeviceId: PHONE.id,
    });
    expect(restored.getState().scenario.devices).toEqual([
      {
        ...PHONE,
        name: "Updated phone",
        type: "tablet",
        latencyMs: 44,
        lanIp: "192.168.1.25",
        osVersion: "16",
        model: "Pixel Tablet",
        appVersion: "2.1.0-preview",
        protocolVersion: 3,
        publicIp: "203.0.113.24",
        location: "Berlin, Germany",
      },
    ]);

    restored.getState().removeDevice(PHONE.id);
    expect(createPreviewScenarioStore(window.sessionStorage).getState().scenario.devices)
      .toEqual([]);
    restored.getState().resetToLive();
    expect(createPreviewScenarioStore(window.sessionStorage).getState().scenario.mode)
      .toBe("live");
  });

  it("shows an added device immediately through discovery IPC", async () => {
    const store = createPreviewScenarioStore(null);
    const intercept = createPreviewInterceptor(store);

    store.getState().addDevice(PHONE);

    expect(store.getState().scenario.resources.discovery).toBe("success");
    await expect(intercept("discovered")).resolves.toMatchObject({
      handled: true,
      value: [{ discovery_id: PHONE.id, name: PHONE.name }],
    });
  });

  it("reports an explicit zero size-refusal count for preview sync success", async () => {
    const store = createPreviewScenarioStore(null);
    const intercept = createPreviewInterceptor(store);
    store.getState().addDevice({ ...PHONE, paired: true });

    await expect(intercept("sync_now")).resolves.toMatchObject({
      handled: true,
      value: [{ skipped_too_large: 0 }],
    });
  });

  it("keeps explicit discovery authoritative across environment changes", async () => {
    const store = createPreviewScenarioStore(window.sessionStorage);
    store.getState().addDevice(WINDOWS_PC);
    store.getState().setDaemon("down");
    store.getState().setNetwork("offline");

    const restored = createPreviewScenarioStore(window.sessionStorage);
    const intercept = createPreviewInterceptor(restored);

    expect(restored.getState().scenario.resources.discovery).toBe("success");
    await expect(intercept("discovered")).resolves.toMatchObject({
      handled: true,
      value: [{ discovery_id: WINDOWS_PC.id }],
    });
    await expect(intercept("rescan")).resolves.toMatchObject({
      handled: true,
      value: [{ discovery_id: WINDOWS_PC.id }],
    });

    restored.getState().resetToLive();
    await expect(intercept("rescan")).resolves.toEqual({ handled: false });
  });

  it("preserves Windows details and exact 31/32 ms latency in the discovered DTO", async () => {
    vi.spyOn(Date, "now").mockReturnValue(2_000_000);
    const store = createPreviewScenarioStore(null);
    const intercept = createPreviewInterceptor(store);

    store.getState().addDevice(WINDOWS_PC);

    await expect(intercept("discovered")).resolves.toEqual({
      handled: true,
      value: [{
        discovery_id: "windows-1",
        name: "Studio PC",
        addr: "192.168.1.31:49232",
        last_seen_ms: 1_996_000,
        paired: false,
        details: {
          profile: {
            display_name: "Studio PC",
            app_version: "2.0.0-alpha.32",
            protocol_version: 2,
            platform: "windows",
            device_class: "desktop",
            os_name: "Windows",
            os_version: "11 24H2",
            model: "Surface Studio 2+",
            provenance: "self_reported",
            trust: "unverified",
            observed_at_ms: 1_996_000,
            fresh_until_ms: 2_011_000,
          },
          endpoint: {
            lan_endpoint: "192.168.1.31:49232",
            provenance: "observed",
            trust: "unverified",
            observed_at_ms: 1_996_000,
            fresh_until_ms: 2_011_000,
          },
          latency: {
            connect_latency_ms: 31,
            provenance: "measured",
            trust: "local",
            observed_at_ms: 1_996_000,
            fresh_until_ms: 2_011_000,
          },
          presence: {
            online: true,
            last_seen_ms: 1_996_000,
            provenance: "observed",
            trust: "local",
            observed_at_ms: 1_996_000,
            fresh_until_ms: 2_011_000,
          },
          public_ip: {
            availability: "available",
            value: "203.0.113.31",
            provenance: "self_reported",
            trust: "unverified",
            observed_at_ms: 1_996_000,
            fresh_until_ms: 2_011_000,
          },
          geo: {
            availability: "available",
            value: "Kyiv, Ukraine",
            provenance: "self_reported",
            trust: "unverified",
            observed_at_ms: 1_996_000,
            fresh_until_ms: 2_011_000,
          },
        },
      }],
    });

    store.getState().updateDevice(WINDOWS_PC.id, { latencyMs: 32 });
    await expect(intercept("rescan")).resolves.toMatchObject({
      handled: true,
      value: [{ details: { latency: { connect_latency_ms: 32 } } }],
    });
  });

  it("marks unpaired claims unverified and absent external data unavailable", async () => {
    const store = createPreviewScenarioStore(null);
    const intercept = createPreviewInterceptor(store);

    store.getState().addDevice(PHONE);

    await expect(intercept("discovered")).resolves.toMatchObject({
      handled: true,
      value: [{
        details: {
          profile: {
            os_name: "Android",
            os_version: "15",
            model: "Pixel 9",
            provenance: "self_reported",
            trust: "unverified",
          },
          endpoint: {
            provenance: "observed",
            trust: "unverified",
          },
          latency: {
            provenance: "measured",
            trust: "local",
          },
          presence: {
            provenance: "observed",
            trust: "local",
          },
          public_ip: { availability: "unavailable" },
          geo: { availability: "unavailable" },
        },
      }],
    });
  });

  it("intercepts resource, daemon, and network states at the IPC boundary", async () => {
    const store = createPreviewScenarioStore(null);
    const intercept = createPreviewInterceptor(store);
    store.getState().setDaemon("up");
    store.getState().addDevice(PHONE);
    store.getState().setResource("discovery", "success");
    store.getState().setResource("history", "loading");

    const pendingHistory = intercept("list");
    store.getState().setResource("history", "empty");

    await expect(intercept("status")).resolves.toEqual({
      handled: true,
      value: expect.objectContaining({ item_count: 0 }),
    });
    await expect(pendingHistory).resolves.toEqual({
      handled: true,
      value: {
        items: [],
        total: 0,
        skipped_undecryptable: 0,
        next_cursor: null,
      },
    });
    await expect(intercept("discovered")).resolves.toMatchObject({
      handled: true,
      value: [{
        discovery_id: PHONE.id,
        name: PHONE.name,
        addr: "192.168.1.24:49200",
        paired: false,
      }],
    });

    store.getState().setNetwork("offline");
    await expect(intercept("rescan")).resolves.toMatchObject({
      handled: true,
      value: [{ discovery_id: PHONE.id }],
    });

    store.getState().setResource("discovery", "live");
    await expect(intercept("rescan")).rejects.toMatchObject({
      code: "peer_unreachable",
      retryable: true,
    });
  });

  it("maps every pairing phase and applies lifecycle command transitions", async () => {
    const store = createPreviewScenarioStore(null);
    const intercept = createPreviewInterceptor(store);
    store.getState().setDaemon("down");

    await expect(intercept("service_state")).resolves.toEqual({
      handled: true,
      value: { state: "stopped" },
    });
    await expect(intercept("start_service")).resolves.toEqual({
      handled: true,
      value: { state: "unhealthy" },
    });
    expect(store.getState().scenario.daemon).toBe("starting");

    store.getState().setDaemon("up");
    store.getState().addDevice(PHONE);
    store.getState().setResource("discovery", "success");

    store.getState().startPairing(PHONE.id);
    expect(store.getState().scenario).toMatchObject({
      pairing: "invite",
      pairingDeviceId: PHONE.id,
    });
    await expect(intercept("pair_progress")).resolves.toMatchObject({
      handled: true,
      value: { state: "waiting_for_peer" },
    });

    store.getState().setPairing("confirm");
    await expect(intercept("pair_progress")).resolves.toMatchObject({
      handled: true,
      value: { state: "awaiting_confirmation" },
    });
    await expect(intercept("pair_confirm")).resolves.toMatchObject({
      handled: true,
      value: { state: "confirmed" },
    });
    expect(store.getState().scenario.devices[0]?.paired).toBe(true);
    await expect(intercept("peers")).resolves.toMatchObject({
      handled: true,
      value: [{ pairing_id: PHONE.id, name: PHONE.name }],
    });
    await expect(intercept("discovered")).resolves.toEqual({
      handled: true,
      value: [],
    });

    store.getState().updateDevice(PHONE.id, { paired: false });
    store.getState().startPairing(PHONE.id);
    await expect(intercept("pair_reject")).resolves.toMatchObject({
      handled: true,
      value: { state: "failed" },
    });
    expect(store.getState().scenario.devices[0]?.paired).toBe(false);
    await expect(intercept("discovered")).resolves.toMatchObject({
      handled: true,
      value: [{ discovery_id: PHONE.id }],
    });

    store.getState().startPairing(PHONE.id);
    await expect(intercept("pair_cancel")).resolves.toMatchObject({
      handled: true,
      value: { state: "cancelled" },
    });
    expect(store.getState().scenario.devices[0]?.paired).toBe(false);
    await expect(intercept("discovered")).resolves.toMatchObject({
      handled: true,
      value: [{ discovery_id: PHONE.id }],
    });

    const expected: Record<PreviewPairingPhase, PairingCeremony["state"]> = {
      idle: "idle",
      invite: "waiting_for_peer",
      code: "waiting_for_peer",
      confirm: "awaiting_confirmation",
      connecting: "handshaking",
      success: "confirmed",
      failure: "failed",
      cancelled: "cancelled",
    };

    for (const phase of Object.keys(expected) as PreviewPairingPhase[]) {
      store.getState().setPairing(phase, PHONE.id);
      const result = await intercept("pair_progress");
      expect(result.handled).toBe(true);
      if (result.handled) {
        expect((result.value as PairingCeremony).state).toBe(expected[phase]);
      }
    }

    await expect(intercept("pair_preview_invite")).resolves.toMatchObject({
      handled: true,
      value: { ceremony: { state: "waiting_for_peer" }, code: "482 916" },
    });
  });
});
