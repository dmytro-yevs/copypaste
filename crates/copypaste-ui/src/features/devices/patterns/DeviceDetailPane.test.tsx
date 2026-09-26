import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { UNKNOWN_DEVICE_IDENTITY } from "@/features/devices/model/devicePresentation";
import { syncReadinessMessage, type SyncBlocked } from "@/features/devices/model";
import { peer } from "@/test/harness";
import { DeviceDetailPane } from "./DeviceDetailPane";

function renderPeer(readiness: SyncBlocked | "ready") {
  const onSync = vi.fn();
  const onRecoverSync = vi.fn();
  render(
    <DeviceDetailPane
      target={{
        kind: "peer",
        name: "Kitchen Mac",
        identity: UNKNOWN_DEVICE_IDENTITY,
        status: { icon: "circle", label: "Waiting", tone: "neutral", busy: false, a11y: {} },
        peer: peer(),
        lastSyncAt: null,
        lastManualSync: null,
      }}
      syncing={false}
      syncReadiness={readiness}
      unpairing={false}
      revoking={false}
      compact={false}
      onSync={onSync}
      onRecoverSync={onRecoverSync}
      onUnpair={vi.fn()}
      onRevoke={vi.fn()}
    />,
  );
  return { onSync, onRecoverSync };
}

describe("peer detail sync readiness", () => {
  it("allows a ready peer sync", () => {
    const { onSync } = renderPeer("ready");
    fireEvent.click(screen.getByRole("button", { name: "Sync now" }));
    expect(onSync).toHaveBeenCalledOnce();
  });

  it.each([
    "service-loading",
    "service-unavailable",
    "config-loading",
    "config-unavailable",
    "disabled",
    "peers-loading",
    "peers-unavailable",
    "no-peers",
  ] as const)("describes and blocks %s", (readiness) => {
    const { onSync, onRecoverSync } = renderPeer(readiness);
    const button = screen.getByRole("button", { name: "Sync now" });
    expect(button.hasAttribute("disabled")).toBe(true);
    const reasonId = button.getAttribute("aria-describedby");
    expect(document.getElementById(reasonId ?? "")?.textContent).toContain(syncReadinessMessage(readiness));
    fireEvent.click(button);
    expect(onSync).not.toHaveBeenCalled();

    if (["service-unavailable", "config-unavailable", "peers-unavailable", "disabled"].includes(readiness)) {
      fireEvent.click(screen.getByRole("button", {
        name: readiness === "disabled" ? "Open Device sync settings" : "Try again",
      }));
      expect(onRecoverSync).toHaveBeenCalledOnce();
    }
  });
});
