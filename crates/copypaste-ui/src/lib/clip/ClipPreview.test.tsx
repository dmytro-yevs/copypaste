import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { HistoryEntry } from "../ipc";
import { ClipPreview, MASKED_A11Y_LABEL } from "./ClipPreview";
import { ContentTile } from "./ContentTile";
import { ClipMetadata } from "./ClipMetadata";

afterEach(cleanup);

const entry = (over: Partial<HistoryEntry> = {}): HistoryEntry => ({
  id: "1",
  content_type: "text/plain",
  preview: "super-secret-token-abc123",
  is_sensitive: true,
  wall_time: 1_700_000_000_000,
  pinned: false,
  ...over,
});

describe("ClipPreview — X6 sensitive masking contract", () => {
  it("masked: real text stays in the DOM (width preserved) but is aria-hidden", () => {
    render(<ClipPreview entry={entry()} masked onReveal={() => {}} />);
    const mask = document.querySelector(".mask");
    expect(mask).not.toBeNull();
    // Real text is present (so the mask occupies real rendered width + is
    // selectable) — but aria-hidden so its accessible name never leaks.
    expect(mask).toHaveTextContent("super-secret-token-abc123");
    expect(mask).toHaveAttribute("aria-hidden", "true");
  });

  it("masked: clicking the mask reveals (calls onReveal)", () => {
    const onReveal = vi.fn();
    render(<ClipPreview entry={entry()} masked onReveal={onReveal} />);
    fireEvent.click(document.querySelector(".mask") as HTMLElement);
    expect(onReveal).toHaveBeenCalledTimes(1);
  });

  it("not masked: renders plain text with no mask element", () => {
    render(<ClipPreview entry={entry()} masked={false} onReveal={() => {}} />);
    expect(document.querySelector(".mask")).toBeNull();
    expect(screen.getByText("super-secret-token-abc123")).toBeInTheDocument();
  });

  it("exposes a placeholder accessible label constant for consumers", () => {
    expect(MASKED_A11Y_LABEL).toMatch(/hidden/i);
    expect(MASKED_A11Y_LABEL).not.toMatch(/secret|token|abc123/i);
  });
});

describe("ContentTile", () => {
  it("renders a decorative (aria-hidden) glyph tile for a text kind", () => {
    render(<ContentTile kind="text" />);
    const tile = document.querySelector(".tile");
    expect(tile).not.toBeNull();
    expect(tile).toHaveAttribute("aria-hidden", "true");
    expect(tile?.querySelector("svg")).not.toBeNull();
  });

  it("renders a swatch for a color with a value", () => {
    render(<ContentTile kind="color" colorValue="#ff0000" />);
    const tile = document.querySelector(".tile--swatch") as HTMLElement | null;
    expect(tile).not.toBeNull();
    expect(tile?.style.background).toContain("rgb(255, 0, 0)");
  });
});

describe("ClipMetadata", () => {
  it("shows the type-word and does not throw for a bare entry", () => {
    render(<ClipMetadata entry={entry({ kind: "URL", is_sensitive: false })} ownDeviceId="dev-1" />);
    expect(screen.getByText("URL")).toBeInTheDocument();
  });
});
