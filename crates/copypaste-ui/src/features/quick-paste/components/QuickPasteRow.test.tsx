import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { QuickPasteRow } from "@/features/quick-paste/components/QuickPasteRow";
import { TooltipProvider } from "@/components/ui";
import { quickPastePresentation } from "@/features/quick-paste/model/quickPastePresentation";
import { item } from "@/test/harness";

const unsupported = item({
  content: "https://future.example/raw",
  content_type: "application/x-future",
  content_class: "other",
});

describe("QuickPasteRow", () => {
  it("keeps unsupported payload text out of its body and copy label", () => {
    render(
      <TooltipProvider>
        <QuickPasteRow
          item={unsupported}
          active
          previewLines={2}
          shortcut={null}
          pinPending={false}
          origin={null}
          fullContent={null}
          fullContentFailed={false}
          onSelect={() => {}}
          onCopy={() => {}}
          onTogglePin={() => {}}
        />
      </TooltipProvider>,
    );

    expect(screen.getByText("Unsupported clipboard content")).not.toBeNull();
    expect(screen.queryByText(unsupported.content!)).toBeNull();
    expect(screen.getByRole("button", { name: "Copy Unsupported clipboard content" })).not.toBeNull();
  });

  it("uses the localized unsupported label", () => {
    expect(quickPastePresentation(unsupported).rowLabel).toBe("Unsupported clipboard content");
  });

  it.each(["", "   "])("keeps a %j finding redaction out of the row label and DOM", (redacted_preview) => {
    const raw = "raw secret fragment";
    render(
      <TooltipProvider>
        <QuickPasteRow item={item({ content: raw, sensitive_finding: { label: "possible token", spans: [], spans_truncated: false, redacted_preview } })} active previewLines={2} shortcut={null} pinPending={false} origin={null} fullContent={null} fullContentFailed={false} onSelect={() => {}} onCopy={() => {}} onTogglePin={() => {}} />
      </TooltipProvider>,
    );

    expect(screen.getByRole("button", { name: "Copy Empty item" })).toBeTruthy();
    expect(screen.queryByText(raw)).toBeNull();
    expect(screen.queryByLabelText(raw)).toBeNull();
  });

  it("uses the resolved full body in the tooltip while keeping the card preview", async () => {
    const user = userEvent.setup();
    render(
      <TooltipProvider>
        <QuickPasteRow item={item({ content: "short preview", truncated: true })} active previewLines={2} shortcut={null} pinPending={false} origin={null} fullContent="complete body" fullContentFailed={false} onSelect={() => {}} onCopy={() => {}} onTogglePin={() => {}} />
      </TooltipProvider>,
    );

    expect(screen.getByText("short preview")).toBeTruthy();
    await user.hover(screen.getByRole("button", { name: "Copy short preview" }));
    expect(await screen.findByText("complete body")).toBeTruthy();
  });

  it.each([
    {
      name: "pending",
      target: item({ content: "short preview", truncated: true }),
      failed: false,
      expected: "Loading the complete value…",
    },
    {
      name: "unavailable",
      target: item({ content: "short preview", truncated: true }),
      failed: true,
      expected: "The complete value could not be loaded.",
    },
  ])("shows the resolved $name state instead of a preview fragment", async ({ target, failed, expected }) => {
    const user = userEvent.setup();
    render(
      <TooltipProvider>
        <QuickPasteRow item={target} active previewLines={2} shortcut={null} pinPending={false} origin={null} fullContent={null} fullContentFailed={failed} onSelect={() => {}} onCopy={() => {}} onTogglePin={() => {}} />
      </TooltipProvider>,
    );

    await user.hover(screen.getByRole("button", { name: "Copy short preview" }));
    expect((await screen.findByRole("status")).textContent).toBe(expected);
  });

  it("keeps a potential-sensitive failed body out of the card and tooltip", async () => {
    const user = userEvent.setup();
    const raw = "raw secret fragment";
    render(
      <TooltipProvider>
        <QuickPasteRow item={item({ content: raw, truncated: true, sensitive_finding: { label: "possible token", spans: [], spans_truncated: false, redacted_preview: "••••• fragment" } })} active previewLines={2} shortcut={null} pinPending={false} origin={null} fullContent={null} fullContentFailed onSelect={() => {}} onCopy={() => {}} onTogglePin={() => {}} />
      </TooltipProvider>,
    );

    expect(screen.queryByText(raw)).toBeNull();
    expect(screen.getByText("Potentially sensitive")).toBeTruthy();
    await user.hover(screen.getByRole("button", { name: "Copy ••••• fragment" }));
    expect((await screen.findByRole("status")).textContent).toBe("The complete value could not be loaded.");
    expect(screen.queryByText(raw)).toBeNull();
  });
});
