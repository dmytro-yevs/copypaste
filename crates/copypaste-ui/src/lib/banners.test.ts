/**
 * INV-17 / AT-25 — exactly one banner, and the loser appears when the winner
 * clears without needing to be re-triggered.
 */
import { describe, expect, it } from "vitest";

import { pickBanner } from "./banners";

const ALL = {
  serviceOffline: true,
  protocolMismatch: 7,
  capturePaused: true,
  dismissed: [] as string[],
};

describe("the banner queue", () => {
  it("shows only the highest-priority condition", () => {
    expect(pickBanner(ALL)?.id).toBe("service-offline");
  });

  it("promotes the next one when the winner clears, with no re-trigger", () => {
    expect(pickBanner({ ...ALL, serviceOffline: false })?.id).toBe(
      "protocol-mismatch",
    );
    expect(
      pickBanner({ ...ALL, serviceOffline: false, protocolMismatch: null })?.id,
    ).toBe("capture-paused");
  });

  it("renders nothing when everything is fine", () => {
    expect(
      pickBanner({
        serviceOffline: false,
        protocolMismatch: null,
        capturePaused: false,
        dismissed: [],
      }),
    ).toBeNull();
  });

  it("skips a dismissed banner but keeps showing the next one", () => {
    const picked = pickBanner({
      ...ALL,
      serviceOffline: false,
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
      { ...ALL, serviceOffline: false, protocolMismatch: null },
    ]) {
      const banner = pickBanner(conditions);
      expect(banner?.message).toBeDefined();
      expect(banner?.message).not.toMatch(/\/Users\/|\/home\/|\.sock/);
    }
  });

  it("names the version in the mismatch copy, because that is the actionable part", () => {
    const banner = pickBanner({
      serviceOffline: false,
      protocolMismatch: 7,
      capturePaused: false,
      dismissed: [],
    });
    expect(banner?.message).toContain("v7");
  });
});
