import { afterEach, describe, expect, it, vi } from "vitest";

import {
  FLEX_GAP_UNSUPPORTED_CLASS,
  applyFlexGapSupportState,
  flexGapQaForcesUnsupported,
  supportsFlexGap,
} from "@/lib/flexGapSupport";

afterEach(() => {
  vi.restoreAllMocks();
  document.documentElement.classList.remove(FLEX_GAP_UNSUPPORTED_CLASS);
  delete document.documentElement.dataset.flexGap;
});

describe("flex gap support", () => {
  it("measures flex layout instead of trusting CSS.supports", () => {
    const supports = vi.spyOn(CSS, "supports").mockReturnValue(true);
    vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(0);

    expect(supportsFlexGap()).toBe(false);
    expect(supports).not.toHaveBeenCalled();
    expect(document.querySelector('[style*="row-gap"]')).toBeNull();
  });

  it("sets one inspectable root state before rendering", () => {
    const scrollHeight = vi
      .spyOn(HTMLElement.prototype, "scrollHeight", "get")
      .mockReturnValueOnce(1)
      .mockReturnValueOnce(2)
      .mockReturnValueOnce(2)
      .mockReturnValue(0);
    expect(applyFlexGapSupportState()).toBe(true);
    expect(document.documentElement.dataset.flexGap).toBe("supported");
    expect(document.documentElement.classList).not.toContain(FLEX_GAP_UNSUPPORTED_CLASS);

    expect(applyFlexGapSupportState()).toBe(true);
    expect(document.documentElement.dataset.flexGap).toBe("supported");

    expect(applyFlexGapSupportState()).toBe(false);
    expect(document.documentElement.dataset.flexGap).toBe("unsupported");
    expect(document.documentElement.classList).toContain(FLEX_GAP_UNSUPPORTED_CLASS);

    expect(flexGapQaForcesUnsupported("?qa-flex-gap=unsupported")).toBe(true);
    expect(
      applyFlexGapSupportState(
        document,
        flexGapQaForcesUnsupported("?qa-flex-gap=unsupported"),
      ),
    ).toBe(false);
    expect(document.documentElement.dataset.flexGap).toBe("unsupported");
    expect(scrollHeight).toHaveBeenCalledTimes(5);
  });
});
