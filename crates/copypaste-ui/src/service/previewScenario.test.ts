import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { PAIRING_SEMANTICS_BY_STATE, type PairingCeremony } from "@/lib/ipc";
import { DEVICE_PRESENCE_OPTIONS } from "@/devtools/PreviewScenarioControls";
import { previewObservedPresence } from "@/service/previewDeviceDto";
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
  presence: "online",
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
  presence: "online",
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

const STORAGE_KEY = "copypaste.dev.preview-scenario";

function persistedScenario(devices: readonly unknown[]) {
  return {
    mode: "scenario",
    daemon: "live",
    network: "live",
    resources: {
      history: "live",
      discovery: "success",
      logs: "live",
      cloud: "live",
    },
    pairing: "idle",
    pairingDeviceId: null,
    devices,
  };
}

function legacyDevice(device: PreviewDevice, online: boolean) {
  const { presence: _presence, ...legacy } = device;
  return { ...legacy, online };
}

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
    expect(
      JSON.parse(window.sessionStorage.getItem(STORAGE_KEY) ?? "{}"),
    ).toMatchObject({ version: 2 });
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

  it("migrates only unambiguous current legacy presence and rejects invalid storage", () => {
    window.sessionStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        version: 1,
        scenario: persistedScenario([
          legacyDevice(PHONE, true),
          legacyDevice({ ...PHONE, id: "legacy-offline" }, false),
          legacyDevice(
            { ...PHONE, id: "legacy-stale", lastSeenAgeMs: 15_001 },
            true,
          ),
        ]),
      }),
    );

    expect(
      createPreviewScenarioStore(window.sessionStorage).getState().scenario
        .devices,
    ).toEqual([
      PHONE,
      { ...PHONE, id: "legacy-offline", presence: "unknown" },
      {
        ...PHONE,
        id: "legacy-stale",
        lastSeenAgeMs: 15_001,
        presence: "unknown",
      },
    ]);

    window.sessionStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        version: 3,
        scenario: persistedScenario([PHONE]),
      }),
    );
    expect(
      createPreviewScenarioStore(window.sessionStorage).getState().scenario
        .mode,
    ).toBe("live");

    window.sessionStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        version: 2,
        scenario: persistedScenario([legacyDevice(PHONE, true)]),
      }),
    );
    expect(
      createPreviewScenarioStore(window.sessionStorage).getState().scenario
        .mode,
    ).toBe("live");
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
            state: "online",
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

  it.each(["online", "offline"] as const)(
    "fails closed to unknown when a %s preview discovery observation is stale",
    async (presence) => {
      vi.spyOn(Date, "now").mockReturnValue(2_000_000);
      const store = createPreviewScenarioStore(null);
      const intercept = createPreviewInterceptor(store);

      store.getState().addDevice({ ...PHONE, presence, lastSeenAgeMs: 15_001 });

      await expect(intercept("discovered")).resolves.toMatchObject({
        handled: true,
        value: [{ details: { presence: { state: "unknown" } } }],
      });
    },
  );

  it.each(["online", "offline", "unknown"] as const)(
    "preserves a current generated %s presence",
    async (presence) => {
      vi.spyOn(Date, "now").mockReturnValue(2_000_000);
      const store = createPreviewScenarioStore(null);
      const intercept = createPreviewInterceptor(store);

      store.getState().addDevice({ ...PHONE, presence });

      await expect(intercept("discovered")).resolves.toMatchObject({
        handled: true,
        value: [{ details: { presence: { state: presence } } }],
      });
    },
  );

  it("fails closed for future, missing, and unbounded observations", () => {
    const now = 2_000_000;
    expect(previewObservedPresence("online", now + 1, now + 15_000, now)).toBe(
      "unknown",
    );
    expect(
      previewObservedPresence("offline", undefined, now + 15_000, now),
    ).toBe("unknown");
    expect(previewObservedPresence("online", now, null, now)).toBe("unknown");
  });

  it("derives compatibility online only from the emitted presence details", async () => {
    vi.spyOn(Date, "now").mockReturnValue(2_000_000);
    const store = createPreviewScenarioStore(null);
    const intercept = createPreviewInterceptor(store);
    for (const presence of ["online", "offline", "unknown"] as const) {
      store.getState().addDevice({
        ...PHONE,
        id: `paired-${presence}`,
        presence,
        paired: true,
      });
    }
    store.getState().addDevice({
      ...PHONE,
      id: "paired-stale-offline",
      presence: "offline",
      paired: true,
      lastSeenAgeMs: 15_001,
    });

    await expect(intercept("peers")).resolves.toMatchObject({
      handled: true,
      value: [
        {
          pairing_id: "paired-online",
          online: true,
          details: { presence: { state: "online" } },
        },
        {
          pairing_id: "paired-offline",
          online: false,
          details: { presence: { state: "offline" } },
        },
        {
          pairing_id: "paired-unknown",
          online: false,
          details: { presence: { state: "unknown" } },
        },
        {
          pairing_id: "paired-stale-offline",
          online: false,
          details: { presence: { state: "unknown" } },
        },
      ],
    });
  });

  it("offers all generated presence states to preview add and edit controls", () => {
    expect(DEVICE_PRESENCE_OPTIONS).toEqual(["online", "offline", "unknown"]);
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

  it("serves bounded History fixtures without falling through fake resource ids", async () => {
    const store = createPreviewScenarioStore(null);
    const intercept = createPreviewInterceptor(store);
    store.getState().setDaemon("up");
    store.getState().setResource("history", "success");

    await expect(intercept("list")).resolves.toMatchObject({
      handled: true,
      value: {
        total: 4,
        items: [
          {
            id: "preview-plain",
            source_app_bundle_id: null,
            source_app_name: null,
          },
          {
            id: "preview-source",
            source_app_bundle_id: "com.example.editor",
            source_app_name: "Example Editor",
          },
          {
            id: "preview-image",
            content_class: "image",
            content: null,
          },
          {
            id: "preview-unknown",
            content_class: "archive",
            truncated: true,
          },
        ],
      },
    });
    await expect(intercept("status")).resolves.toMatchObject({
      handled: true,
      value: { item_count: 4 },
    });
    await expect(intercept("get_item_body", { id: "preview-plain" })).resolves.toEqual({
      handled: true,
      value: "Preview clipboard content from a fixture.",
    });
    await expect(intercept("get_item_body", { id: "preview-source" })).resolves.toEqual({
      handled: true,
      value: "Fixture body from Example Editor.",
    });
    await expect(intercept("get_image_preview", { id: "preview-image" })).resolves.toEqual({
      handled: true,
      value: {
        width: 1,
        height: 1,
        png_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
      },
    });
    await expect(intercept("get_item_body", { id: "preview-unknown" })).rejects.toMatchObject({
      code: "unsupported_content",
      retryable: false,
    });
    await expect(intercept("get_item_body", { id: "not-a-preview-item" })).rejects.toMatchObject({
      code: "not_found",
      retryable: false,
    });
    for (const id of ["toString", "constructor", "__proto__"]) {
      await expect(intercept("get_item_body", { id })).rejects.toMatchObject({
        code: "not_found",
        retryable: false,
      });
    }
    await expect(intercept("get_image_preview", { id: "not-a-preview-image" })).rejects.toMatchObject({
      code: "not_found",
      retryable: false,
    });

    for (const state of ["empty", "loading", "error"] as const) {
      store.getState().setResource("history", state);
      if (state === "error") {
        await expect(intercept("list")).rejects.toMatchObject({ code: "internal", retryable: true });
      } else if (state === "loading") {
        const pending = intercept("list");
        store.getState().setResource("history", "empty");
        await expect(pending).resolves.toMatchObject({
          handled: true,
          value: { total: 0, items: [] },
        });
      } else {
        await expect(intercept("list")).resolves.toMatchObject({
          handled: true,
          value: { total: 0, items: [] },
        });
      }
      await expect(intercept("status")).resolves.toMatchObject({
        handled: true,
        value: { item_count: 0 },
      });
      await expect(intercept("get_item_body", { id: "not-a-preview-item" })).resolves.toEqual({ handled: false });
    }

    store.getState().setResource("history", "success");
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    await expect(intercept("get_item_body", { id: "preview-plain" })).resolves.toEqual({ handled: false });
    delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;

    store.getState().resetToLive();
    await expect(intercept("get_item_body", { id: "preview-plain" })).resolves.toEqual({ handled: false });
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
      value: { state: "rejected", semantics: PAIRING_SEMANTICS_BY_STATE.rejected },
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
      rejected: "rejected",
      timed_out: "timed_out",
      failure: "failed",
      cancelled: "cancelled",
    };
    const expectedMessage: Record<
      PreviewPairingPhase,
      PairingCeremony["semantics"]["message_id"]
    > = {
      idle: "ready",
      invite: "waiting_for_peer",
      code: "waiting_for_peer",
      confirm: "compare_codes",
      connecting: "securing_connection",
      success: "paired",
      rejected: "rejected",
      timed_out: "timed_out",
      failure: "failed",
      cancelled: "cancelled",
    };

    for (const phase of Object.keys(expected) as PreviewPairingPhase[]) {
      store.getState().setPairing(phase, PHONE.id);
      const result = await intercept("pair_progress");
      expect(result.handled).toBe(true);
      if (result.handled) {
        const ceremony = result.value as PairingCeremony;
        expect(ceremony.state).toBe(expected[phase]);
        expect(ceremony.semantics.message_id).toBe(expectedMessage[phase]);
      }
    }

    await expect(intercept("pair_preview_invite")).resolves.toMatchObject({
      handled: true,
      value: { ceremony: { state: "waiting_for_peer" }, code: "482 916" },
    });
  });
});
