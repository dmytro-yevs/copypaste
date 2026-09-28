import { describe, expect, it } from "vitest";

import { resolveHistoryContent } from "./resolveHistoryContent";

describe("resolveHistoryContent", () => {
  it("keeps existing rows ahead of loading and failed polling", () => {
    expect(resolveHistoryContent({ loading: true, hasItems: true, errorKind: "offline", privateMode: false, filtered: false })).toBe("list");
  });

  it("shows service failure ahead of private and filtered empty states", () => {
    expect(resolveHistoryContent({ loading: false, hasItems: false, errorKind: "offline", privateMode: true, filtered: true })).toBe("offline");
  });
});
