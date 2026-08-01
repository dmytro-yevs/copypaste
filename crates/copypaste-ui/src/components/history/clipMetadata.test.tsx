import { describe, expect, it } from "vitest";

import { clipSourceMetadata } from "@/components/history/clipMetadata";
import { item } from "@/test/harness";

describe("clipSourceMetadata", () => {
  it("uses an app-specific label and icon for known source applications", () => {
    const source = clipSourceMetadata(
      item({ source_app_bundle_id: "com.google.Chrome" }),
    );

    expect(source.label).toBe("Google Chrome");
    expect(source.available).toBe(true);
  });

  it("turns a valid package identifier into a safe display label", () => {
    const source = clipSourceMetadata(
      item({ source_app_bundle_id: "dev.copypaste.clipboard_helper" }),
    );

    expect(source.label).toBe("Clipboard Helper");
    expect(source.available).toBe(true);
  });

  it("prefers the platform-reported display name over an inferred identifier label", () => {
    const source = clipSourceMetadata(
      item({
        source_app_bundle_id: "com.example.writer",
        source_app_name: "Writer Pro",
      }),
    );

    expect(source.label).toBe("Writer Pro");
    expect(source.available).toBe(true);
  });

  it("keeps an attributed helper's platform-reported name when macOS has no bundle id", () => {
    const source = clipSourceMetadata(item({ source_app_name: "Codex Helper" }));

    expect(source.label).toBe("Codex Helper");
    expect(source.available).toBe(true);
  });

  it("never treats a source id as an icon URL", () => {
    const source = clipSourceMetadata(
      item({ source_app_bundle_id: "file:///Applications/Untrusted.app" }),
    );

    expect(source.label).toBe("Unknown app");
    expect(source.available).toBe(false);
  });
});
