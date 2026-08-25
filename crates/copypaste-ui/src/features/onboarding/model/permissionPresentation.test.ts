import { describe, expect, it } from "vitest";

import type { OnboardingPermissionStatus } from "@/lib/ipc";
import { permissionPresentation } from "./permissionPresentation";

describe("permissionPresentation", () => {
  it.each<
    [OnboardingPermissionStatus, ReturnType<typeof permissionPresentation>]
  >([
    ["prompt", { action: "request", label: "request", explanation: "default", disabled: false }],
    ["granted", { action: "none", label: "granted", explanation: "default", disabled: true }],
    ["denied", { action: "open-settings", label: "open-settings", explanation: "denied", disabled: false }],
    ["not_required", { action: "none", label: "not-required", explanation: "not-required", disabled: true }],
    ["unavailable", { action: "none", label: "unavailable", explanation: "unavailable", disabled: true }],
  ])("maps %s exhaustively", (status, expected) => {
    expect(permissionPresentation(status)).toEqual(expected);
  });
});
