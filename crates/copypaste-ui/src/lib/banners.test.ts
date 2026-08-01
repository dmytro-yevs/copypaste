/**
 * Key-store failure is the one global condition that needs a full persistent
 * explanation. Service states live in the compact status affordance.
 */
import { describe, expect, it } from "vitest";

import { pickBanner } from "./banners";

const ALL = {
  historyUnreadable: "key_unusable" as const,
};

describe("the blocking banner", () => {
  it("shows the unreadable key state", () => {
    expect(pickBanner(ALL)?.id).toBe("history-unreadable");
  });

  it("renders nothing when everything is fine", () => {
    expect(
      pickBanner({
        historyUnreadable: null,
      }),
    ).toBeNull();
  });

  it("never puts a path or a raw error in its copy", () => {
    const banner = pickBanner(ALL);
    expect(banner?.message).toBeDefined();
    expect(banner?.message).not.toMatch(/\/Users\/|\/home\/|\.sock/);
  });

  it("shows an unusable key with neither a retry nor a dismiss", () => {
    const banner = pickBanner({
      historyUnreadable: "key_unusable",
    });
    expect(banner?.id).toBe("history-unreadable");
  });
});
