import { describe, expect, it } from "vitest";

import type { OnboardingPermissionStatus } from "@/lib/ipc";
import { permissionPresentation } from "./permissionPresentation";

describe("permissionPresentation", () => {
  it.each<
    [OnboardingPermissionStatus, ReturnType<typeof permissionPresentation>]
  >([
    ["prompt", { action: "request", label: "request", disabled: false }],
    ["granted", { action: "none", label: "granted", disabled: true }],
    ["denied", { action: "open-settings", label: "open-settings", disabled: false }],
    ["not_required", { action: "none", label: "not-required", disabled: true }],
    ["unavailable", { action: "none", label: "unavailable", disabled: true }],
  ])("maps %s exhaustively", (status, expected) => {
    expect(permissionPresentation(status)).toEqual(expected);
  });
});
