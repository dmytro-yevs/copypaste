import { describe, expect, it } from "vitest";

import { IpcFailure } from "@/lib/errors";
import type { PairingCeremony, PairingSemantics } from "@/lib/ipc";
import {
  pairingClientErrorPresentation,
  pairingIsActive,
  pairingPresentation,
} from "./pairingPresentation";

function ceremony(semantics: PairingSemantics): PairingCeremony {
  return {
    ceremony_id: "ceremony-1",
    role: "initiator",
    state: "failed",
    semantics,
    presentation: "presented",
    known_device: null,
    error: null,
  };
}

describe("pairingPresentation", () => {
  it("uses generated semantic facts for copy, icon, live region, and actions", () => {
    const semantics: PairingSemantics = {
      message_id: "compare_codes",
      icon: "shieldCheck",
      tone: "warning",
      live: "status",
      active: true,
      terminal: false,
      needs_devices: true,
      review_secure: true,
      retry: false,
      copy: {
        title: "Compare security codes",
        detail: "Confirm the code in the native security prompt.",
      },
    };

    expect(pairingPresentation(ceremony(semantics))).toMatchObject({
      semantics,
      title: "Compare security codes",
      detail: "Confirm the code in the native security prompt.",
    });
    expect(pairingIsActive(ceremony(semantics))).toBe(true);
  });

  it("keeps terminal failure descriptors distinct without local state maps", () => {
    const ids = ["timed_out", "cancelled", "rejected", "code_mismatch"] as const;
    for (const message_id of ids) {
      const presentation = pairingPresentation(
        ceremony({
          message_id,
          icon: message_id === "cancelled" || message_id === "rejected" ? "close" : "alert",
          tone: message_id === "cancelled" ? "neutral" : "warning",
          live: message_id === "cancelled" ? "status" : "alert",
          active: false,
          terminal: true,
          needs_devices: false,
          review_secure: false,
          retry: true,
          copy: { title: `Status ${message_id}`, detail: `Recover ${message_id}` },
        }),
      );
      expect(presentation.title).toContain(message_id);
      expect(presentation.semantics.retry).toBe(true);
    }
  });

  it("does not invent retry for oversized or unknown client failures", () => {
    expect(
      pairingClientErrorPresentation(new IpcFailure("content_too_large", true))?.retry,
    ).toBe(false);
    expect(
      pairingClientErrorPresentation(new IpcFailure("future_code", true))?.retry,
    ).toBe(false);
    expect(
      pairingClientErrorPresentation(new IpcFailure("peer_unreachable", true))?.retry,
    ).toBe(true);
  });
});
