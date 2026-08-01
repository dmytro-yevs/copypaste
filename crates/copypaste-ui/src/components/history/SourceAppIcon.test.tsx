import { afterEach, describe, expect, it, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { AppWindow } from "lucide-react";

import { SourceAppIcon } from "@/components/history/SourceAppIcon";
import { getSourceAppIcon } from "@/lib/ipc";

vi.mock("@/lib/ipc", () => ({
  getSourceAppIcon: vi.fn(),
}));

const getIcon = vi.mocked(getSourceAppIcon);
const originalCreateObjectURL = URL.createObjectURL;
const originalRevokeObjectURL = URL.revokeObjectURL;

afterEach(() => {
  getIcon.mockReset();
  URL.createObjectURL = originalCreateObjectURL;
  URL.revokeObjectURL = originalRevokeObjectURL;
});

describe("SourceAppIcon", () => {
  it("renders the native icon returned for a captured source bundle id", async () => {
    getIcon.mockResolvedValue({
      // A valid one-pixel PNG. The WebView receives this from native Rust, not
      // an application path or a renderer-invented URL.
      png_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL1xQAAAABJRU5ErkJggg==",
      width: 1,
      height: 1,
    });
    URL.createObjectURL = vi.fn(() => "blob:source-icon") as typeof URL.createObjectURL;
    URL.revokeObjectURL = vi.fn() as typeof URL.revokeObjectURL;

    const { container } = render(
      <SourceAppIcon
        bundleId="com.google.Chrome"
        Fallback={AppWindow}
      />,
    );

    await waitFor(() => expect(getIcon).toHaveBeenCalledWith("com.google.Chrome"));
    const icon = await waitFor(() => {
      const resolved = container.querySelector<HTMLImageElement>("[data-source-app-icon]");
      expect(resolved).not.toBeNull();
      return resolved!;
    });
    expect(icon.getAttribute("src")).toBe("blob:source-icon");
    expect(icon.className).toContain("size-4");
  });

  it("uses the semantic fallback only when native resolution has no icon", async () => {
    getIcon.mockResolvedValue(null);
    const { container } = render(
      <SourceAppIcon bundleId="com.example.missing" Fallback={AppWindow} />,
    );

    await waitFor(() => expect(getIcon).toHaveBeenCalled());
    expect(container.querySelector("[data-source-app-icon]")).toBeNull();
  });
});
