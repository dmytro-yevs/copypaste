import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { QuickPasteRow } from "@/features/quick-paste/components/QuickPasteRow";
import { TooltipProvider } from "@/components/ui";
import { quickPastePresentation } from "@/features/quick-paste/model/quickPastePresentation";
import { item, testClient } from "@/test/harness";
import { QueryClientProvider } from "@tanstack/react-query";

const clipboardAvailability = vi.hoisted(() => ({ value: "available" }));
vi.mock("@/hooks/useClipboardWriteAvailability", () => ({
  useClipboardWriteAvailability: () => ({ isPending: false, isError: false, data: clipboardAvailability.value }),
}));

const unsupported = item({
  content: "https://future.example/raw",
  content_type: "application/x-future",
  content_class: "other",
});

describe("QuickPasteRow", () => {
  beforeEach(() => { clipboardAvailability.value = "available"; });

  it("anchors copy and unavailable select tooltips to their actionable buttons", async () => {
    const user = userEvent.setup();
    const target = item({ content: "copy target" });
    const props = {
      item: target, active: true, shortcut: null, pinPending: false,
      origin: null, fullContent: null, fullContentFailed: false,
      onSelect: () => {}, onSelectFromKeyboard: () => {},
      onCopy: () => {}, onTogglePin: () => {},
    };
    const { rerender } = render(<TooltipProvider><QuickPasteRow {...props} /></TooltipProvider>);
    const copy = screen.getByRole("button", { name: "Copy copy target" });
    await user.hover(copy);
    expect(await screen.findByRole("tooltip", { name: "Copy copy target" })).toBeTruthy();
    expect(copy.getAttribute("aria-describedby")).not.toBeNull();

    clipboardAvailability.value = "unsupported_on_platform";
    rerender(<TooltipProvider><QuickPasteRow {...props} /></TooltipProvider>);
    const select = screen.getByRole("button", { name: "Select copy target" });
    await user.hover(select);
    expect(await screen.findByRole("tooltip", { name: "Select copy target" })).toBeTruthy();
    expect(select.getAttribute("aria-describedby")).not.toBeNull();
  });

  it("selects an unavailable row by touch or keyboard without invoking copy", async () => {
    clipboardAvailability.value = "unsupported_on_platform";
    const onSelect = vi.fn();
    const onSelectFromKeyboard = vi.fn();
    const onCopy = vi.fn();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteRow
          item={item({ content: null, content_type: "image/png", content_class: "image" })}
          active
          shortcut="⌘1"
          pinPending={false}
          origin={null}
          fullContent={null}
          fullContentFailed={false}
          onSelect={onSelect}
          onSelectFromKeyboard={onSelectFromKeyboard}
          onCopy={onCopy}
          onTogglePin={() => {}}
        /></TooltipProvider>
      </QueryClientProvider>,
    );

    const copy = screen.getByRole("button", { name: "Copy Image" });
    const select = screen.getByRole("button", { name: "Select Image" });
    expect(copy.hasAttribute("disabled")).toBe(true);
    expect(screen.getByText("This clipboard format can’t be copied on this device.")).toBeTruthy();
    expect(screen.queryByText("⌘1")).toBeNull();
    fireEvent.pointerDown(select, { pointerType: "touch", button: 0 });
    fireEvent.pointerUp(select, { pointerType: "touch", button: 0 });
    expect(onSelect).toHaveBeenCalledOnce();
    expect(onCopy).not.toHaveBeenCalled();

    select.focus();
    await userEvent.setup().keyboard("{Enter}");
    expect(onSelectFromKeyboard).toHaveBeenCalledOnce();
    expect(onCopy).not.toHaveBeenCalled();
  });
  it("keeps unsupported payload text out of its body and copy label", () => {
    render(
      <TooltipProvider>
        <QuickPasteRow
          item={unsupported}
          active
          shortcut={null}
          pinPending={false}
          origin={null}
          fullContent={null}
          fullContentFailed={false}
          onSelect={() => {}}
          onSelectFromKeyboard={() => {}}
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

  it("normalizes line breaks for the compact action list", () => {
    render(
      <TooltipProvider>
        <QuickPasteRow item={item({ content: "first line\nsecond line" })} active shortcut={null} pinPending={false} origin={null} fullContent={null} fullContentFailed={false} onSelect={() => {}} onSelectFromKeyboard={() => {}} onCopy={() => {}} onTogglePin={() => {}} />
      </TooltipProvider>,
    );

    expect(screen.getByText("first line second line").getAttribute("data-preview-lines")).toBe("1");
  });

  it("keeps source metadata available to assistive technology", () => {
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider>
          <QuickPasteRow item={item({ source_app_name: "Notes" })} active shortcut={null} pinPending={false} origin={null} fullContent={null} fullContentFailed={false} onSelect={() => {}} onSelectFromKeyboard={() => {}} onCopy={() => {}} onTogglePin={() => {}} />
        </TooltipProvider>
      </QueryClientProvider>,
    );

    expect(screen.getByText("Notes")).toBeTruthy();
  });

  it("uses the pin control as the sole visible pinned indicator", () => {
    const { container } = render(
      <TooltipProvider>
        <QuickPasteRow item={item({ pinned: true, sensitive_finding: { label: "possible token", spans: [], spans_truncated: false, redacted_preview: "••••• fragment" } })} active={false} shortcut={null} pinPending={false} origin={null} fullContent={null} fullContentFailed={false} onSelect={() => {}} onSelectFromKeyboard={() => {}} onCopy={() => {}} onTogglePin={() => {}} />
      </TooltipProvider>,
    );

    expect(screen.queryByText("Pinned")).toBeNull();
    expect(screen.getByRole("button", { name: "Unpin" }).getAttribute("aria-pressed")).toBe("true");
    expect(container.querySelector('[data-tone="warning"]')).toBeTruthy();
    expect(container.querySelector('[role="listitem"]')?.getAttribute("data-pinned")).toBe("true");
  });

  it.each(["", "   "])("keeps a %j finding redaction out of the row label and DOM", (redacted_preview) => {
    const raw = "raw secret fragment";
    render(
      <TooltipProvider>
        <QuickPasteRow item={item({ content: raw, sensitive_finding: { label: "possible token", spans: [], spans_truncated: false, redacted_preview } })} active shortcut={null} pinPending={false} origin={null} fullContent={null} fullContentFailed={false} onSelect={() => {}} onSelectFromKeyboard={() => {}} onCopy={() => {}} onTogglePin={() => {}} />
      </TooltipProvider>,
    );

    expect(screen.getByRole("button", { name: "Copy Empty item" })).toBeTruthy();
    expect(screen.queryByText(raw)).toBeNull();
    expect(screen.queryByLabelText(raw)).toBeNull();
  });

  it("keeps the resolved full body out of the compact row", () => {
    render(
      <TooltipProvider>
        <QuickPasteRow item={item({ content: "short preview", truncated: true })} active shortcut={null} pinPending={false} origin={null} fullContent="complete body" fullContentFailed={false} onSelect={() => {}} onSelectFromKeyboard={() => {}} onCopy={() => {}} onTogglePin={() => {}} />
      </TooltipProvider>,
    );

    expect(screen.getByText("short preview")).toBeTruthy();
    expect(screen.queryByText("complete body")).toBeNull();
  });

  it("keeps a potential-sensitive failed body out of the compact row", () => {
    const raw = "raw secret fragment";
    render(
      <TooltipProvider>
        <QuickPasteRow item={item({ content: raw, truncated: true, sensitive_finding: { label: "possible token", spans: [], spans_truncated: false, redacted_preview: "••••• fragment" } })} active shortcut={null} pinPending={false} origin={null} fullContent={null} fullContentFailed onSelect={() => {}} onSelectFromKeyboard={() => {}} onCopy={() => {}} onTogglePin={() => {}} />
      </TooltipProvider>,
    );

    expect(screen.queryByText(raw)).toBeNull();
    expect(screen.getByText("Potentially sensitive")).toBeTruthy();
    expect(screen.queryByText(raw)).toBeNull();
  });
});
