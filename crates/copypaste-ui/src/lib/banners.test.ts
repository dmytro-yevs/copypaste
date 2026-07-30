/**
 * INV-17 / AT-25 — exactly one banner, and the loser appears when the winner
 * clears without needing to be re-triggered.
 */
import { describe, expect, it } from "vitest";

import { legacyHistoryPresent, pickBanner } from "./banners";

const ALL = {
  serviceOffline: true,
  historyUnreadable: "legacy_database" as const,
  protocolMismatch: 7,
  capturePaused: true,
  legacyHistory: true,
  dismissed: [] as string[],
};

describe("the banner queue", () => {
  it("shows only the highest-priority condition", () => {
    expect(pickBanner(ALL)?.id).toBe("service-offline");
  });

  it("promotes the next one when the winner clears, with no re-trigger", () => {
    expect(pickBanner({ ...ALL, serviceOffline: false })?.id).toBe(
      "history-unreadable",
    );
    expect(
      pickBanner({ ...ALL, serviceOffline: false, historyUnreadable: null })?.id,
    ).toBe("protocol-mismatch");
    expect(
      pickBanner({
        ...ALL,
        serviceOffline: false,
        historyUnreadable: null,
        protocolMismatch: null,
      })?.id,
    ).toBe("capture-paused");
  });

  it("renders nothing when everything is fine", () => {
    expect(
      pickBanner({
        serviceOffline: false,
        historyUnreadable: null,
        protocolMismatch: null,
        capturePaused: false,
        legacyHistory: false,
        dismissed: [],
      }),
    ).toBeNull();
  });

  it("skips a dismissed banner but keeps showing the next one", () => {
    const picked = pickBanner({
      ...ALL,
      serviceOffline: false,
      historyUnreadable: null,
      dismissed: ["protocol-mismatch"],
    });
    expect(picked?.id).toBe("capture-paused");
  });

  it("will not let the P0 be dismissed", () => {
    // A user cannot act on the app at all while the service is down, so
    // dismissing the notice would only hide the reason nothing works.
    const picked = pickBanner({ ...ALL, dismissed: ["service-offline"] });
    expect(picked?.id).toBe("service-offline");
    expect(picked?.dismissible).toBe(false);
  });

  it("never puts a path or a raw error in its copy", () => {
    for (const conditions of [
      ALL,
      { ...ALL, serviceOffline: false },
      { ...ALL, serviceOffline: false, historyUnreadable: "key_unusable" as const },
      { ...ALL, serviceOffline: false, historyUnreadable: null },
      {
        ...ALL,
        serviceOffline: false,
        historyUnreadable: null,
        protocolMismatch: null,
      },
    ]) {
      const banner = pickBanner(conditions);
      expect(banner?.message).toBeDefined();
      expect(banner?.message).not.toMatch(/\/Users\/|\/home\/|\.sock/);
    }
  });

  it("names the version in the mismatch copy, because that is the actionable part", () => {
    const banner = pickBanner({
      serviceOffline: false,
      historyUnreadable: null,
      protocolMismatch: 7,
      capturePaused: false,
      legacyHistory: false,
      dismissed: [],
    });
    expect(banner?.message).toContain("v7");
  });

  /**
   * The banner is the only thing carrying this news on the Devices and
   * Settings tabs, and it must not carry a control either — there is no way
   * out of an unreadable history, and a dismiss would only hide the reason
   * nothing on any tab works.
   */
  it.each(["legacy_database", "key_unusable"] as const)(
    "shows %s with neither a retry nor a dismiss",
    (kind) => {
      const banner = pickBanner({
        serviceOffline: false,
        historyUnreadable: kind,
        protocolMismatch: null,
        capturePaused: false,
        legacyHistory: false,
        dismissed: ["history-unreadable"],
      });
      expect(banner?.id).toBe("history-unreadable");
      expect(banner?.action).toBeUndefined();
      expect(banner?.dismissible).toBe(false);
      expect(banner?.severity).toBe("error");
    },
  );

  it("tells the two unreadable conditions apart", () => {
    const base = {
      serviceOffline: false,
      protocolMismatch: null,
      capturePaused: false,
      legacyHistory: false,
      dismissed: [],
    };
    const legacy = pickBanner({ ...base, historyUnreadable: "legacy_database" });
    const unusable = pickBanner({ ...base, historyUnreadable: "key_unusable" });
    expect(legacy?.message).not.toBe(unusable?.message);
    expect(legacy?.message).toMatch(/0\.4/);
  });
});

/**
 * CLAUDE.md rule 3's second obligation, at the surface: a v0.4 history is
 * *found* and the user is *told*. Nothing is broken while it holds — v2 has
 * started a new history and the old one is untouched — which is why it is last
 * in the queue and why it can be dismissed.
 */
describe("a CopyPaste 0.4 history sitting beside the new one", () => {
  const quiet = {
    serviceOffline: false,
    historyUnreadable: null,
    protocolMismatch: null,
    capturePaused: false,
    legacyHistory: true,
    dismissed: [] as string[],
  };

  it("says the old history is still there, not only that it cannot be read", () => {
    const banner = pickBanner(quiet);
    expect(banner?.id).toBe("legacy-history");
    expect(banner?.severity).toBe("info");
    // The reassurance is the point. Without it an empty list reads as loss.
    expect(banner?.message).toMatch(/still on this device/i);
    expect(banner?.message).toMatch(/unchanged/i);
  });

  it("yields to anything actually broken", () => {
    expect(pickBanner({ ...quiet, capturePaused: true })?.id).toBe("capture-paused");
    expect(pickBanner({ ...quiet, serviceOffline: true })?.id).toBe("service-offline");
  });

  it("can be dismissed, unlike the conditions that stop the app working", () => {
    expect(pickBanner(quiet)?.dismissible).toBe(true);
    expect(pickBanner({ ...quiet, dismissed: ["legacy-history"] })).toBeNull();
  });

  it("names no path", () => {
    expect(pickBanner(quiet)?.message).not.toMatch(/\/Users\/|\/home\/|~\/|\.db\b/);
  });
});

describe("reading the flag off the wire", () => {
  it("is true only for an explicit true", () => {
    expect(legacyHistoryPresent({ legacy_history_present: true })).toBe(true);
    expect(legacyHistoryPresent({ legacy_history_present: false })).toBe(false);
    // A daemon built before the field simply omits it, and `undefined` must not
    // become a banner.
    expect(legacyHistoryPresent({ version: "2.0.0" })).toBe(false);
    expect(legacyHistoryPresent(undefined)).toBe(false);
    expect(legacyHistoryPresent(null)).toBe(false);
  });
});
