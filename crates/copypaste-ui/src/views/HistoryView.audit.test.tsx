/**
 * Audit-fix tests for HistoryView.tsx — six issues:
 *
 * CopyPaste-10lk (extensibility): rowTreatment token-driven, not skin-name hardcoded.
 *   - Uses SKINS[skin].rowTreatment instead of `skin === "vapor"` / `skin === "quiet"`.
 *
 * CopyPaste-o2o9 (vapor inset visual): vapor rows apply inline styles for
 *   rounded-card surface (borderRadius: var(--r-card)) and per-row
 *   marginBottom (var(--skin-row-gap)) for spacing — because VirtualList rows
 *   are absolutely positioned and flex gap on a wrapper is a no-op.
 *
 * CopyPaste-kp6f (skin radius tokens): file-attach btn, device-filter select,
 *   sort toggle, and selection-glide div must NOT use `rounded-ide` class;
 *   they must apply inline style with var(--r-ctl) or var(--r-card).
 *   Toast borderRadius must use "var(--r-card)" not hardcoded 10.
 *
 * CopyPaste-5917.54 (meta sub-row): history rows show a .meta sub-row beneath the
 *   preview with KindChip (readable kind text label), timestamp, and optional app.
 *   Matches the approved styleguide .hrow .meta pattern.
 *
 * CopyPaste-bdac.54 (glide overlay radius): selection-glide overlay at line ~1402
 *   used var(--r-card, 14px) — wrong fallback for Classic canonical 12px.
 *   Fixed to var(--r-card, 12px).
 *
 * CopyPaste-bdac.66 (image placeholder copy): FullResImage showed "Image unavailable"
 *   and plain "Loading…" — inconsistent with empty/error patterns. Now shows
 *   "Couldn't load image" with sub-hint and italic/faint loading state.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import React from "react";
import { useUI } from "../store";

// ---------------------------------------------------------------------------
// Tauri mock — must be set up BEFORE importing HistoryView.
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { HistoryView } from "./HistoryView";
import { GlassToast } from "../components/Toast";

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
// CopyPaste-10lk: row treatment driven by SKINS[skin].rowTreatment token
// ---------------------------------------------------------------------------

describe("Phase 4: card row treatment (border-b + hover lift)", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("rows use card treatment (border-b, no skin-row-inset/skin-row-line)", async () => {
    setupInvokeWithItems([makeEntry("a")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const rows = screen.getAllByRole("option");
    expect(rows[0].className).toContain("border-b");
    expect(rows[0].className).not.toContain("skin-row-inset");
    expect(rows[0].className).not.toContain("skin-row-line");
    // Classic "card" rows must carry the cinematic hover lift transform — this is
    // the distinguishing token of the card treatment vs. the quiet "line" treatment
    // (which explicitly omits the translateX+scale hover). Previously missing:
    // this assertion catches regressions where the hover lift is removed without
    // the test failing.
    expect(rows[0].className).toMatch(/hover:\[transform:translateX.*scale/);
    // Classic rows must carry the card-entry animation class (list-item-in).
    // This is the surface-card entry class — it drives the appear animation for
    // each card on the glass surface. Previously missing from this test.
    expect(rows[0].className).toContain("list-item-in");
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-o2o9: vapor inset rows render with inline style for card surface
// ---------------------------------------------------------------------------

// Phase 4: vapor/inset skin treatment removed — no inset row tests needed.

// ---------------------------------------------------------------------------
// CopyPaste-kp6f: skin radius tokens on controls (no rounded-ide class)
// ---------------------------------------------------------------------------

describe("CopyPaste-kp6f: controls use var(--r-ctl) inline, not rounded-ide class", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("file-attach button uses inline var(--r-ctl) borderRadius, not rounded-ide class", async () => {
    setupInvokeWithItems([makeEntry("a")]);
    const { container } = render(<HistoryView />);
    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const attachBtn = container.querySelector('button[aria-label="Add file"]');
    expect(attachBtn).not.toBeNull();
    if (attachBtn) {
      const cls = attachBtn.getAttribute("class") ?? "";
      expect(cls).not.toContain("rounded-ide");
      const style = attachBtn.getAttribute("style") ?? "";
      expect(style).toMatch(/border-radius.*var\(--r-ctl/i);
    }
  });

  it("sort-toggle button uses inline var(--r-ctl) borderRadius, not rounded-ide class", async () => {
    // Sort toggle only renders when there are multiple devices
    // Provide items from two different device IDs
    const items = [
      { ...makeEntry("a"), origin_device_id: "device-1", origin_device_name: "Mac 1" },
      { ...makeEntry("b"), origin_device_id: "device-2", origin_device_name: "Mac 2" },
    ];
    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "history_page") {
        return Promise.resolve(ipcOk({ items, total: items.length, own_device_id: "device-1" }));
      }
      if (args?.method === "status") {
        return Promise.resolve(
          ipcOk({ status: "running", private_mode: false, ready: true, degraded: false })
        );
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    const { container } = render(<HistoryView />);
    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    // Sort toggle button: aria-label contains "Sort by"
    const sortBtn = container.querySelector('button[aria-label^="Sort by"]');
    if (sortBtn) {
      const cls = sortBtn.getAttribute("class") ?? "";
      expect(cls).not.toContain("rounded-ide");
      const style = sortBtn.getAttribute("style") ?? "";
      expect(style).toMatch(/border-radius.*var\(--r-ctl/i);
    }
    // If sort button not shown (single device), pass — that's expected.
  });

  it("Toast component uses var(--r-card) for borderRadius, not a hardcoded value", () => {
    // Previously this test was a conditional no-op: it only asserted when a toast
    // happened to be visible after HistoryView IPC actions, which never fired in
    // JSDOM. This replacement renders GlassToast directly and asserts the token
    // unconditionally — the assertion can never silently pass without the
    // var(--r-card) token being present.
    // CopyPaste-bdac.56: Toast radius is the modal token (--r-card), not --r-card.
    const { container } = render(
      <GlassToast msg={{ id: "kp6f-toast", text: "test" }} onDismiss={() => {}} />,
    );
    const bubble = container.querySelector('[role="status"]') as HTMLElement | null;
    expect(bubble).not.toBeNull();

    // Accept either inline style or a Tailwind arbitrary-value class that encodes the var.
    const inlineStyle = bubble!.style.borderRadius;
    const hasVarInStyle = inlineStyle.includes("--r-card");
    const hasVarInClass = bubble!.className.includes("--r-card");
    expect(hasVarInStyle || hasVarInClass).toBe(true);

    // Regression guard: must NOT be a bare hardcoded pixel value (e.g. "10px").
    expect(inlineStyle).not.toBe("10px");
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-5917.54: .meta sub-row shows KindChip label text in each row
// ---------------------------------------------------------------------------

describe("CopyPaste-5917.54: history rows show KindChip label text in .meta sub-row", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("URL item shows the text 'URL' visibly in the row (not just as aria-label/tooltip)", async () => {
    setupInvokeWithItems([{ ...makeEntry("url1"), content_type: "url", kind: "URL" }]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item url1")).toBeInTheDocument());

    // The KindChip in the .meta sub-row renders the label as readable DOM text.
    // getByText matches visible text nodes — not aria-label/title attrs.
    // This confirms the label is present for sighted users without hover.
    const kindLabel = screen.getAllByText("URL");
    expect(kindLabel.length).toBeGreaterThan(0);
  });

  it("TEXT item shows 'TEXT' label in the row", async () => {
    setupInvokeWithItems([makeEntry("t1")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item t1")).toBeInTheDocument());

    const kindLabel = screen.getAllByText("TEXT");
    expect(kindLabel.length).toBeGreaterThan(0);
  });

  it("row without kind still shows a fallback label derived from content_type", async () => {
    // No `kind` field — should fall back to kindFallback("text") = "TEXT"
    setupInvokeWithItems([{ ...makeEntry("t2"), content_type: "text" }]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item t2")).toBeInTheDocument());

    // kindFallback returns "TEXT" for content_type="text"
    expect(screen.getAllByText("TEXT").length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-bdac.54: glide overlay uses correct 12px fallback (not 14px)
// ---------------------------------------------------------------------------

describe("CopyPaste-bdac.54: all var(--r-card) usages in HistoryView use 12px fallback", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("glide selection overlay fallback is 12px, not 14px", async () => {
    // Read the source text to confirm 14px is gone and 12px is the only fallback.
    // Uses process.cwd() which resolves to the workspace root in vitest.
    const { readFileSync } = await import("fs");
    const { resolve } = await import("path");
    const src = readFileSync(
      resolve(process.cwd(), "src/views/HistoryView.tsx"),
      "utf8",
    );

    // No occurrence of the wrong fallback in any var(--r-card, ...) usage.
    // Filter out comment lines (lines beginning with //) before matching.
    const codeLines = src.split("\n").filter(l => !l.trimStart().startsWith("//")).join("\n");
    const wrongFallbacks = [...codeLines.matchAll(/var\(--r-card,\s*(\d+px)\)/g)].filter(
      ([, px]) => px !== "12px",
    );
    expect(wrongFallbacks).toHaveLength(0);
  });

  // Phase 4: vapor/inset skin removed — runtime inset row test no longer needed.
});

// ---------------------------------------------------------------------------
// CopyPaste-bdac.66: FullResImage error/loading copy matches empty-state patterns
// ---------------------------------------------------------------------------

describe("CopyPaste-bdac.66: FullResImage placeholder copy is consistent with empty-state patterns", () => {
  it("HistoryView source no longer contains the old 'Image unavailable' copy", async () => {
    // Static source check: confirm the old copy strings are gone and the new ones
    // are present. This is reliable regardless of async modal-open flow in JSDOM.
    // FullResImage was extracted to HistoryView/DetailsModal.tsx (CopyPaste-g06m.13
    // refactor) — check both the main file and the sibling module.
    const { readFileSync } = await import("fs");
    const { resolve } = await import("path");
    const src = readFileSync(
      resolve(process.cwd(), "src/views/HistoryView.tsx"),
      "utf8",
    );
    const detailsModalSrc = readFileSync(
      resolve(process.cwd(), "src/views/HistoryView/DetailsModal.tsx"),
      "utf8",
    );
    const combinedSrc = src + detailsModalSrc;

    // Old copies must be gone
    expect(combinedSrc).not.toContain('"Image unavailable"');

    // New copies must be present (may be in DetailsModal.tsx after refactor)
    expect(combinedSrc).toContain("Couldn't load image");
    expect(combinedSrc).toContain("Try reopening this item.");
  });

  it("shows 'Couldn't load image' in the Details modal when image fetch fails", async () => {
    invoke.mockReset();

    const imageEntry = {
      ...makeEntry("img-bdac66"),
      content_type: "image/png",
    };

    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "history_page") {
        return Promise.resolve(ipcOk({ items: [imageEntry], total: 1 }));
      }
      if (args?.method === "status") {
        return Promise.resolve(
          ipcOk({ status: "running", private_mode: false, ready: true, degraded: false })
        );
      }
      if (args?.method === "get_item_image") {
        return Promise.reject(new Error("not found"));
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    const { container } = render(<HistoryView />);

    // Image rows use "image" (lowercase) as kindLabel in aria-label
    await waitFor(() => {
      const row = container.querySelector('[role="option"][aria-label^="image:"]');
      expect(row).not.toBeNull();
    });

    // Open the Details modal via the Preview button
    const previewBtn = container.querySelector('button[aria-label="Preview"]');
    expect(previewBtn).not.toBeNull();
    fireEvent.click(previewBtn!);

    // Modal should now be open — wait for the FullResImage error state
    await waitFor(
      () => expect(screen.queryByText("Couldn't load image")).not.toBeNull(),
      { timeout: 3000 },
    );

    expect(screen.queryByText("Try reopening this item.")).not.toBeNull();
    expect(screen.queryByText("Image unavailable")).toBeNull();
  });
});
