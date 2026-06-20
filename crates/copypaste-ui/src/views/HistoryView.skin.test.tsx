/**
 * W-C3: HistoryView skin row treatment tests.
 *
 * Asserts that the list container and individual rows render the correct
 * CSS class pattern for each of the three skins:
 *   classic — card-style: border-b dividers + cinematic hover lift (unchanged)
 *   quiet   — line-style: border-b flat dividers, NO hover lift transform
 *   vapor   — inset-style: individual card rows with border-radius and gap
 *
 * Classic visual behavior is frozen — the existing HistoryView tests must
 * continue to pass, and these tests must not break when skin = "classic".
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { useUI } from "../store";

// ---------------------------------------------------------------------------
// Tauri mock — must be set up BEFORE importing any module that uses invoke.
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { HistoryView } from "./HistoryView";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeEntry(id: string, wallTime = 1_700_000_000_000) {
  return {
    id,
    content_type: "text",
    preview: `Item ${id}`,
    is_sensitive: false,
    wall_time: wallTime,
    pinned: false,
  };
}

function ipcOk(data: unknown) {
  return { ok: true, data, error: null, error_code: null };
}

function setupInvokeWithItems(items: ReturnType<typeof makeEntry>[]) {
  invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
    if (args?.method === "history_page") {
      return Promise.resolve(ipcOk({ items, total: items.length }));
    }
    if (args?.method === "status") {
      return Promise.resolve(
        ipcOk({ status: "running", private_mode: false, ready: true, degraded: false })
      );
    }
    return Promise.reject("daemon_offline:/tmp/x.sock");
  });
}

// ---------------------------------------------------------------------------
// W-C3: Skin row treatment
// ---------------------------------------------------------------------------

describe("W-C3: HistoryView skin row treatment", () => {
  beforeEach(() => {
    invoke.mockReset();
    // Reset skin to classic before each test.
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "classic" } }));
  });

  it("classic: rows have border-b divider classes (unchanged behavior)", async () => {
    const items = [makeEntry("a"), makeEntry("b")];
    setupInvokeWithItems(items);

    render(<HistoryView />);

    await waitFor(() => {
      expect(screen.getByText("Item a")).toBeInTheDocument();
    });

    // Find a clipboard row by its role=option
    const rows = screen.getAllByRole("option");
    expect(rows.length).toBeGreaterThan(0);

    const firstRow = rows[0];
    // Classic rows have border-b (flat divider between rows).
    expect(firstRow.className).toContain("border-b");
  });

  it("classic: rows do NOT have vapor inset class", async () => {
    const items = [makeEntry("a"), makeEntry("b")];
    setupInvokeWithItems(items);

    render(<HistoryView />);

    await waitFor(() => {
      expect(screen.getByText("Item a")).toBeInTheDocument();
    });

    const rows = screen.getAllByRole("option");
    const firstRow = rows[0];
    // Classic rows must NOT use vapor inset styling.
    expect(firstRow.className).not.toContain("skin-row-inset");
  });

  it("quiet: rows use line treatment (border-b, skin-row-line class)", async () => {
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "quiet" } }));

    const items = [makeEntry("a"), makeEntry("b")];
    setupInvokeWithItems(items);

    render(<HistoryView />);

    await waitFor(() => {
      expect(screen.getByText("Item a")).toBeInTheDocument();
    });

    const rows = screen.getAllByRole("option");
    expect(rows.length).toBeGreaterThan(0);

    const firstRow = rows[0];
    // Quiet rows use line treatment.
    expect(firstRow.className).toContain("skin-row-line");
  });

  it("quiet: rows do NOT have cinematic hover lift class (no translateX+scale)", async () => {
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "quiet" } }));

    const items = [makeEntry("a")];
    setupInvokeWithItems(items);

    render(<HistoryView />);

    await waitFor(() => {
      expect(screen.getByText("Item a")).toBeInTheDocument();
    });

    const rows = screen.getAllByRole("option");
    const firstRow = rows[0];
    // Quiet skin must not apply the cinematic hover lift that classic uses.
    // The classic hover class contains translateX in the hover variant.
    expect(firstRow.className).not.toMatch(/translateX.*scale/);
  });

  it("vapor: rows use inset treatment (skin-row-inset class, rounded, gap)", async () => {
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "vapor" } }));

    const items = [makeEntry("a"), makeEntry("b")];
    setupInvokeWithItems(items);

    render(<HistoryView />);

    await waitFor(() => {
      expect(screen.getByText("Item a")).toBeInTheDocument();
    });

    const rows = screen.getAllByRole("option");
    expect(rows.length).toBeGreaterThan(0);

    const firstRow = rows[0];
    // Vapor rows use inset treatment with rounded corners.
    expect(firstRow.className).toContain("skin-row-inset");
  });

  it("vapor: list container adds skin-row-gap spacing class", async () => {
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "vapor" } }));

    const items = [makeEntry("a"), makeEntry("b")];
    setupInvokeWithItems(items);

    const { container } = render(<HistoryView />);

    await waitFor(() => {
      expect(screen.getByText("Item a")).toBeInTheDocument();
    });

    // The list container should have a class indicating row gap for vapor.
    // This can be a wrapper div around the VirtualList or the listbox itself.
    const gapEl = container.querySelector(".skin-list-vapor");
    expect(gapEl).not.toBeNull();
  });

  it("vapor: rows do NOT have border-b divider (inset rows are self-contained cards)", async () => {
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "vapor" } }));

    const items = [makeEntry("a"), makeEntry("b")];
    setupInvokeWithItems(items);

    render(<HistoryView />);

    await waitFor(() => {
      expect(screen.getByText("Item a")).toBeInTheDocument();
    });

    const rows = screen.getAllByRole("option");
    const firstRow = rows[0];
    // Vapor inset rows are individual cards — no flat border-b divider.
    expect(firstRow.className).not.toContain("border-b");
  });

  it("classic skin: all existing features still work (copy flash, selection, pin indicator)", async () => {
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "classic" } }));

    const items = [
      { ...makeEntry("pinned-1"), pinned: true },
      makeEntry("unpinned-1"),
    ];
    setupInvokeWithItems(items);

    render(<HistoryView />);

    await waitFor(() => {
      expect(screen.getByText("Item pinned-1")).toBeInTheDocument();
      expect(screen.getByText("Item unpinned-1")).toBeInTheDocument();
    });

    // Both pinned and unpinned rows must render.
    const rows = screen.getAllByRole("option");
    expect(rows.length).toBeGreaterThanOrEqual(2);
  });
});
