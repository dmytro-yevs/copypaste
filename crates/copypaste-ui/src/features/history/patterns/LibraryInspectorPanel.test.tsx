import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { item } from "@/test/harness";
import { LibraryInspectorPanel } from "./LibraryInspectorPanel";

const callbacks = {
  onReveal: vi.fn(),
  onHide: vi.fn(),
  onCopy: vi.fn(),
  onTogglePin: vi.fn(),
  onDelete: vi.fn(),
  onClose: vi.fn(),
};

function inspector(
  overrides: Partial<Parameters<typeof LibraryInspectorPanel>[0]> = {},
) {
  return (
    <TooltipProvider>
      <LibraryInspectorPanel
        item={item()}
        origin={null}
        revealedContent={null}
        fullContent={null}
        fullContentFailed={false}
        revealPending={false}
        {...callbacks}
        {...overrides}
      />
    </TooltipProvider>
  );
}

describe("LibraryInspectorPanel", () => {
  it("never presents a truncated preview after the full-body read fails", () => {
    render(
      inspector({
        item: item({ content: "short preview", truncated: true }),
        fullContentFailed: true,
      }),
    );

    expect(screen.getByRole("status").textContent).toBe(
      "Full contents could not be loaded.",
    );
    expect(screen.queryByText("short preview")).toBeNull();
  });

  it("uses the canonical preview surface and semantic metadata list", () => {
    const { container } = render(inspector());
    const preview = container.querySelector('[data-slot="preview-surface"]');
    const metadata = container.querySelector<HTMLDListElement>(
      'dl[data-slot="metadata-list"]',
    );

    expect(preview).not.toBeNull();
    expect(metadata?.getAttribute("data-density")).toBe("compact");
    const rows = metadata?.querySelectorAll('[data-slot="metadata-row"]');
    expect(rows?.length).toBeGreaterThan(0);
    for (const row of rows ?? []) {
      expect(
        within(row as HTMLElement).getByRole("term"),
      ).toBeTruthy();
      expect(row.querySelector("dd[data-slot='metadata-value']")).not.toBeNull();
    }
  });

  it("keeps sensitive plaintext out until an ephemeral reveal is supplied", () => {
    const secret = item({ is_sensitive: true });
    const { rerender } = render(inspector({ item: secret }));

    expect(
      screen.getByRole("button", {
        name: "Sensitive content hidden — activate to reveal",
      }),
    ).toBeTruthy();
    expect(screen.queryByText("revealed once")).toBeNull();

    rerender(inspector({ item: secret, revealedContent: "revealed once" }));
    expect(screen.getByText("revealed once")).toBeTruthy();

    rerender(inspector({ item: secret, revealedContent: null }));
    expect(screen.queryByText("revealed once")).toBeNull();
  });
});
