import { afterEach, describe, expect, it, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";

const { getImagePreview } = vi.hoisted(() => ({
  getImagePreview: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({ getImagePreview }));

import { HistoryImagePreview } from "@/components/history/HistoryImagePreview";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  getImagePreview.mockReset();
});

describe("HistoryImagePreview", () => {
  it("renders the daemon thumbnail through a short-lived Blob URL", async () => {
    getImagePreview.mockResolvedValue({
      png_base64: "iVBORw0KGgo=",
      width: 1,
      height: 1,
    });
    const createObjectURL = vi.fn(() => "blob:history-preview");
    const revokeObjectURL = vi.fn();
    vi.stubGlobal("URL", { createObjectURL, revokeObjectURL });

    const { container, unmount } = render(
      <HistoryImagePreview id="clip-1" style={{ width: 64, height: 64 }} />,
    );

    await waitFor(() => expect(container.querySelector("img")).not.toBeNull());
    const image = container.querySelector("img");
    expect(image?.getAttribute("src")).toBe("blob:history-preview");
    expect(image?.classList.contains("object-contain")).toBe(true);
    expect(image?.style.height).toBe("64px");
    expect(createObjectURL).toHaveBeenCalledTimes(1);

    unmount();
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:history-preview");
  });

  it("keeps a non-image fallback when preview generation fails", async () => {
    getImagePreview.mockRejectedValue(new Error("offline"));
    const { container } = render(<HistoryImagePreview id="clip-1" />);

    await waitFor(() => expect(getImagePreview).toHaveBeenCalledWith("clip-1"));
    expect(container.querySelector("img")).toBeNull();
  });
});
