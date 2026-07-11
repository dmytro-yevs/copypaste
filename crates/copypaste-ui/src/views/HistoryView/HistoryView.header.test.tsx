import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { HistoryView } from "../HistoryView";
import type { HistoryEntry } from "../../lib/ipc";

// src/lib/fixtures/** is import-restricted to mockIpc.ts + GalleryView/**
// (see importBoundary.test.ts) — build the fixture entry inline instead,
// matching the pattern used by HistoryRow.test.tsx.
function makeEntry(over: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id: "1",
    content_type: "text/plain",
    preview: "Sample clipboard text",
    is_sensitive: false,
    wall_time: 1_700_000_000_000,
    pinned: false,
    ...over,
  };
}

// ---------------------------------------------------------------------------
// HistoryView header — shortcut-hint declutter (CopyPaste-7w060.6)
//
// The header previously spelled out "⌘F search · ⌘A select all · ⌥⏎ paste as
// plain text" as always-visible copy next to the search field. It read as
// disabled/low-contrast noise and crowded the bar even with zero items.
// Shortcuts must stay discoverable (as a tooltip) without competing visually
// with search + primary commands.
// ---------------------------------------------------------------------------

const { apiMocks } = vi.hoisted(() => ({
  apiMocks: {
    historyPage: vi.fn(),
    status: vi.fn(async () => ({ ok: true, degraded: false })),
    getPrivateMode: vi.fn(async () => ({ private_mode: false })),
  },
}));

vi.mock("../../lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/ipc")>();
  return {
    ...actual,
    api: { ...actual.api, ...apiMocks },
  };
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("HistoryView header — shortcut hints (CopyPaste-7w060.6)", () => {
  it("does not render the spelled-out shortcut hint text at empty state", async () => {
    apiMocks.historyPage.mockResolvedValue({ items: [], total: 0, own_device_id: "dev-1" });
    render(<HistoryView />);

    await screen.findByText("Nothing copied yet");

    expect(screen.queryByText(/⌘F search/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/⌘A select all/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/paste as plain text/i)).not.toBeInTheDocument();
  });

  it("does not render the spelled-out shortcut hint text when items are present", async () => {
    apiMocks.historyPage.mockResolvedValue({
      items: [makeEntry({ id: "1" })],
      total: 1,
      own_device_id: "dev-1",
    });
    render(<HistoryView />);

    await screen.findByText("Sample clipboard text");

    expect(screen.queryByText(/⌘F search/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/⌘A select all/i)).not.toBeInTheDocument();
  });
});
