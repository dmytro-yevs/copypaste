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

describe("cloud settings icon presentation", () => {
  it.each([
    ["loading", undefined, false, true, { icon: "cloud", iconOwner: "device" }],
    ["query failure", undefined, true, false, { icon: "alert", iconOwner: "settings" }],
    ["not configured", status(), false, false, { icon: "cloud", iconOwner: "settings" }],
    ["signed out", status({ configured: true }), false, false, { icon: "cloudOff", iconOwner: "device" }],
    ["healthy", status({ configured: true, signed_in: true, key_ready: true }), false, false, { icon: "shieldCheck", iconOwner: "device" }],
    ["connected attention", status({ configured: true, signed_in: true, key_ready: true, last_error: "failed" }), false, false, { icon: "shieldCheck", iconOwner: "settings" }],
  ] as const)("keeps %s settings icon semantics", (_case, value, failed, loading, expected) => {
    expect(cloudSettingsPresentation(value, failed, loading)).toEqual(expected);
  });
});
