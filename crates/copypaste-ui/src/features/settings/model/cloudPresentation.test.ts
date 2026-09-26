import { describe, expect, it } from "vitest";

import type { CloudStatusData } from "@/lib/ipc";
import { cloudSettingsPresentation } from "./cloudPresentation";

function status(over: Partial<CloudStatusData> = {}): CloudStatusData {
  return {
    configured: false,
    signed_in: false,
    key_ready: false,
    email: null,
    last_sync_ms: null,
    last_error: null,
    poll_interval_secs: 60,
    unreadable_uploads: 0,
    ...over,
  };
}

describe("cloud settings connection presentation", () => {
  it.each([
    ["loading", undefined, false, true, { state: "checking", icon: "cloud", description: "settings.sync.cloud.loading" }],
    ["query failure", undefined, true, false, { state: "unavailable", icon: "cloudOff", description: "settings.sync.cloud.statusUnavailable" }],
    ["missing status", undefined, false, false, { state: "unavailable", icon: "cloudOff", description: "settings.sync.cloud.statusUnavailable" }],
    ["not configured", status(), false, false, {
      state: "not-configured", icon: "cloudOff", description: "settings.sync.cloud.notConfigured",
      badge: { label: "settings.sync.cloud.badgeNotConfigured", variant: "warn" },
    }],
    ["signed out", status({ configured: true }), false, false, {
      state: "signed-out", icon: "cloudOff", description: "settings.sync.cloud.signedOutDescription",
      badge: { label: "settings.sync.cloud.badgeSignedOut", variant: "secondary" },
    }],
    ["healthy", status({ configured: true, signed_in: true, key_ready: true }), false, false, {
      state: "healthy", icon: "shieldCheck", description: "settings.sync.cloud.description",
      badge: { label: "settings.sync.cloud.badgeConnected", variant: "ok" },
    }],
    ["last sync error", status({ configured: true, signed_in: true, key_ready: true, last_error: "failed" }), false, false, {
      state: "attention", icon: "alert", description: "settings.sync.cloud.attentionDescription",
      badge: { label: "settings.sync.cloud.badgeAttention", variant: "warn" },
    }],
    ["unreadable uploads", status({ configured: true, signed_in: true, key_ready: true, unreadable_uploads: 2 }), false, false, {
      state: "attention", icon: "alert", description: "settings.sync.cloud.attentionDescription",
      badge: { label: "settings.sync.cloud.badgeAttention", variant: "warn" },
    }],
  ] as const)("maps canonical %s health to settings copy and tone", (_case, value, failed, loading, expected) => {
    expect(cloudSettingsPresentation(value, failed, loading)).toEqual(expected);
  });

  it("uses immediate sync failure while the last status still appears healthy", () => {
    const healthy = status({ configured: true, signed_in: true, key_ready: true });
    expect(cloudSettingsPresentation(healthy, false, false, true)).toMatchObject({
      state: "attention",
      icon: "alert",
      badge: { label: "settings.sync.cloud.badgeAttention", variant: "warn" },
    });
  });
});
