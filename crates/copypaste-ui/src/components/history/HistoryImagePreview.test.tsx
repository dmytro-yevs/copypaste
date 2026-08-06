import { afterEach, describe, expect, it, vi } from "vitest";
import { waitFor } from "@testing-library/react";

const { getImagePreview, getSourceAppIcon } = vi.hoisted(() => ({
  getImagePreview: vi.fn(),
  getSourceAppIcon: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({ getImagePreview, getSourceAppIcon }));

import { HistoryImagePreview } from "@/components/history/HistoryImagePreview";
import { testClient, withClient } from "@/test/harness";

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

    const { container, unmount } = withClient(
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
    const { container } = withClient(<HistoryImagePreview id="clip-1" />);

    await waitFor(() => expect(getImagePreview).toHaveBeenCalledWith("clip-1"));
    expect(container.querySelector("img")).toBeNull();
  });

  // F-UI-6. The Blob URL is still owned by the component and still revoked on
  // unmount; only the round trip behind it survives the virtualizer recycling
  // the row.
  it("does not re-fetch when a recycled row mounts again", async () => {
    getImagePreview.mockResolvedValue({
      png_base64: "iVBORw0KGgo=",
      width: 1,
      height: 1,
    });
    const createObjectURL = vi.fn(() => "blob:history-preview");
    const revokeObjectURL = vi.fn();
    vi.stubGlobal("URL", { createObjectURL, revokeObjectURL });

    const client = testClient();
    const first = withClient(<HistoryImagePreview id="clip-1" />, client);
    await waitFor(() => expect(first.container.querySelector("img")).not.toBeNull());
    first.unmount();
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:history-preview");

    const again = withClient(<HistoryImagePreview id="clip-1" />, client);
    await waitFor(() => expect(again.container.querySelector("img")).not.toBeNull());
    expect(getImagePreview).toHaveBeenCalledTimes(1);
  });
});
