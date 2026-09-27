import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { item } from "@/test/harness";
import { QuickPastePreview } from "./QuickPastePreview";

vi.mock("@/features/clip-content", () => ({
  ClipImageLoader: () => <img alt="preview image" />,
}));

const layout = { side: "right" as const, width: 320 };

describe("QuickPastePreview", () => {
  it("uses the shared preview renderer for text and images", () => {
    const { rerender } = render(
      <QuickPastePreview item={item({ content: "preview text" })} fullContent={null} fullContentFailed={false} layout={layout} />,
    );
    expect(screen.getByText("preview text")).toBeTruthy();

    rerender(
      <QuickPastePreview item={item({ content: null, content_type: "image/png", content_class: "image" })} fullContent={null} fullContentFailed={false} layout={layout} />,
    );
    expect(screen.getByRole("img", { name: "preview image" })).toBeTruthy();
  });

  it("uses the scrollable reader mode for the complete selected body", () => {
    const full = "first line\nsecond line\nthird line\nfourth line";
    render(
      <QuickPastePreview item={item({ content: "first line", truncated: true })} fullContent={full} fullContentFailed={false} layout={layout} />,
    );

    const reader = screen.getByRole("region", { name: "Clipboard preview" });
    expect(reader.getAttribute("data-mode")).toBe("reader");
    expect(reader.textContent).toContain("fourth line");
  });

  it("never renders sensitive plaintext and keeps findings redacted", () => {
    const raw = "raw secret fragment";
    const { rerender } = render(
      <QuickPastePreview item={item({ content: null, is_sensitive: true })} fullContent={raw} fullContentFailed={false} layout={layout} />,
    );
    expect(screen.queryByText(raw)).toBeNull();

    rerender(
      <QuickPastePreview
        item={item({ content: raw, sensitive_finding: { label: "possible token", spans: [], spans_truncated: false, redacted_preview: "••••• fragment" } })}
        fullContent={raw}
        fullContentFailed={false}
        layout={layout}
      />,
    );
    expect(screen.getByText("••••• fragment")).toBeTruthy();
    expect(screen.queryByText(raw)).toBeNull();
  });
});
