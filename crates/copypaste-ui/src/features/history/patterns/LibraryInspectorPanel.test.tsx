import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { item } from "@/test/harness";
import { LibraryInspectorPanel } from "./LibraryInspectorPanel";
import styles from "./LibraryInspectorPanel.module.css";

vi.mock("@/hooks/useClipboardWriteAvailability", () => ({
  useClipboardWriteAvailability: () => ({ isPending: false, isError: false, data: "available" }),
}));

vi.mock("@/features/source-apps", () => ({
  SourceAppIcon: ({ fallbackText }: { fallbackText?: string }) => (
    <span data-slot="source-app-icon">{fallbackText}</span>
  ),
}));

const inspectorStyles = readFileSync(
  resolve(process.cwd(), "src/features/history/patterns/LibraryInspectorPanel.module.css"),
  "utf8",
);

const callbacks = {
  onReveal: vi.fn(),
  onHide: vi.fn(),
  onCopy: vi.fn(),
  onTogglePin: vi.fn(),
  onDelete: vi.fn(),
  onOpenReader: vi.fn(),
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
  it("keeps a pending sensitive preview masked with the shared loading indicator", () => {
    render(inspector({
      item: item({ content: "private body", is_sensitive: true }),
      fullContent: "private body",
      revealPending: true,
    }));

    const reveal = screen.getByRole("button", { name: "Sensitive content hidden — activate to reveal" });
    expect(reveal.hasAttribute("disabled")).toBe(true);
    expect(reveal.querySelectorAll('[data-mode="loading"][data-placement="control"]')).toHaveLength(1);
    expect(screen.queryByText("private body")).toBeNull();
  });

  it("opens the reader and reveals a potential original only after a gesture", async () => {
    const user = userEvent.setup();
    const target = item({
      content: "original token",
      sensitive_finding: {
        label: "possible token",
        spans: [{ start: 9, end: 14 }],
        spans_truncated: false,
        redacted_preview: "redacted token",
      },
    });
    callbacks.onOpenReader.mockClear();
    render(inspector({ item: target }));

    expect(screen.getByText("Potentially sensitive content")).toBeTruthy();
    expect(screen.getByText("redacted token")).toBeTruthy();
    expect(screen.queryByText("original token")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Show original content" }));
    expect(screen.getByText("original token")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Hide original content" }));
    expect(screen.queryByText("original token")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Show full contents" }));
    expect(callbacks.onOpenReader).toHaveBeenCalledWith(target, expect.any(HTMLElement));
  });

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

  it("keeps the timestamp in the content grid column without a source", () => {
    const { container } = render(inspector());
    const source = container.querySelector<HTMLElement>(`.${styles.source}`);
    const sourceCopy = source?.querySelector<HTMLElement>(
      `.${styles.sourceCopy}`,
    );

    expect(source?.children).toHaveLength(2);
    expect(sourceCopy?.querySelector("small")).not.toBeNull();
    expect(inspectorStyles).toMatch(
      /\.sourceCopy\s*\{[^}]*grid-column:\s*2;/,
    );
  });

  it("keeps source text and timestamp in the content grid column with a source", () => {
    const { container } = render(
      inspector({
        item: item({
          source_app_bundle_id: "com.example.editor",
          source_app_name: "Example Editor",
        }),
      }),
    );
    const source = container.querySelector<HTMLElement>(`.${styles.source}`);
    const sourceCopy = source?.querySelector<HTMLElement>(
      `.${styles.sourceCopy}`,
    );

    expect(source?.children).toHaveLength(3);
    expect(sourceCopy?.querySelector("strong")?.textContent).toBe(
      "Example Editor",
    );
    expect(sourceCopy?.querySelector("small")).not.toBeNull();
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

  it("does not restore Copy focus after the user focuses elsewhere", async () => {
    const user = userEvent.setup();
    const target = item({ id: "focus-first" });
    const view = (copyPending: boolean) => (
      <>
        {inspector({ item: target, copyPending })}
        <button type="button">Elsewhere</button>
      </>
    );
    const { rerender } = render(view(false));
    const copy = screen.getByRole("button", { name: "Copy" });
    copy.focus();
    await user.keyboard("{Enter}");
    rerender(view(true));
    const elsewhere = screen.getByRole("button", { name: "Elsewhere" });
    await user.click(elsewhere);
    rerender(view(false));

    expect(document.activeElement).toBe(elsewhere);
  });

  it("does not refocus Copy after a background pointer gesture", async () => {
    const user = userEvent.setup();
    const target = item({ id: "focus-first" });
    const { rerender } = render(inspector({ item: target }));
    const copy = screen.getByRole("button", { name: "Copy" });
    copy.focus();
    await user.keyboard("{Enter}");
    rerender(inspector({ item: target, copyPending: true }));
    fireEvent.pointerDown(document.body, { button: 0 });
    rerender(inspector({ item: target, copyPending: false }));

    expect(document.activeElement).not.toBe(screen.getByRole("button", { name: "Copy" }));
  });

  it("allows a new Copy attempt when the prior callback never became pending", async () => {
    callbacks.onCopy.mockClear();
    const target = item({ id: "focus-first" });
    const { rerender } = render(inspector({ item: target }));
    const copy = screen.getByRole("button", { name: "Copy" });
    copy.focus();
    fireEvent.click(copy);
    await new Promise<void>((resolve) => window.setTimeout(resolve, 0));

    copy.blur();
    fireEvent.click(copy);
    expect(callbacks.onCopy).toHaveBeenCalledTimes(2);
    rerender(inspector({ item: target, copyPending: true }));
    rerender(inspector({ item: target, copyPending: false }));
    expect(document.activeElement).toBe(document.body);
  });

  it("does not restore Copy focus to another selected item", async () => {
    const user = userEvent.setup();
    const first = item({ id: "focus-first" });
    const second = item({ id: "focus-second" });
    const { rerender } = render(inspector({ item: first }));
    const copy = screen.getByRole("button", { name: "Copy" });
    copy.focus();
    await user.keyboard("{Enter}");
    rerender(inspector({ item: second, copyPending: true }));
    rerender(inspector({ item: second, copyPending: false }));

    expect(document.activeElement).not.toBe(screen.getByRole("button", { name: "Copy" }));
  });
});
