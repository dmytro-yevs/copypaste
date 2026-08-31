import { describe, expect, it } from "vitest";

import type { CloudStatusData } from "@/lib/ipc";
import { cloudConnectionState } from "./cloud";

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

describe("cloud connection state", () => {
  it.each([
    ["loading", undefined, false, true, "checking"],
    ["query failure", undefined, true, false, "unavailable"],
    ["missing status", undefined, false, false, "unavailable"],
    ["not configured", status(), false, false, "not-configured"],
    ["signed out", status({ configured: true }), false, false, "signed-out"],
    ["attention", status({ configured: true, signed_in: true, key_ready: true, last_error: "failed" }), false, false, "attention"],
    ["healthy", status({ configured: true, signed_in: true, key_ready: true }), false, false, "healthy"],
  ] as const)("classifies %s without a screen dependency", (_case, value, failed, loading, expected) => {
    expect(cloudConnectionState(value, failed, loading)).toBe(expected);
  });
});
