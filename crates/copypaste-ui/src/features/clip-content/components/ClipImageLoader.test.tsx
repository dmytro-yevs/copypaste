import { describe, expect, it } from "vitest";

import { previewEdge } from "./ClipImageLoader";
import { imagePreviewKey } from "@/lib/imagePreviewQuery";

describe("image preview resolution", () => {
  it("uses the measured edge and DPR within the native bounds", () => {
    expect(previewEdge(300, 180, 2)).toBe(640);
    expect(previewEdge(1, 1, 1)).toBe(128);
    expect(previewEdge(2_000, 1_000, 2)).toBe(2_048);
  });

  it("keeps different resolutions in distinct cached queries", () => {
    expect(imagePreviewKey("item", 384)).not.toEqual(imagePreviewKey("item", 1024));
  });
});
