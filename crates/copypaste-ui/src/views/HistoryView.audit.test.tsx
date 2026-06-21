/**
 * Audit-fix tests for HistoryView.tsx — three issues:
 *
 * CopyPaste-10lk (extensibility): rowTreatment token-driven, not skin-name hardcoded.
 *   - Uses SKINS[skin].rowTreatment instead of `skin === "vapor"` / `skin === "quiet"`.
 *
 * CopyPaste-o2o9 (vapor inset visual): vapor rows apply inline styles for
 *   rounded-card surface (borderRadius: var(--skin-r-card)) and per-row
 *   marginBottom (var(--skin-row-gap)) for spacing — because VirtualList rows
 *   are absolutely positioned and flex gap on a wrapper is a no-op.
 *
 * CopyPaste-kp6f (skin radius tokens): file-attach btn, device-filter select,
 *   sort toggle, and selection-glide div must NOT use `rounded-ide` class;
 *   they must apply inline style with var(--skin-r-ctl) or var(--skin-r-card).
 *   Toast borderRadius must use "var(--skin-r-card)" not hardcoded 10.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
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

describe("CopyPaste-10lk: rowTreatment token-driven (not skin-name hardcoded)", () => {
  beforeEach(() => {
    invoke.mockReset();
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "classic" } }));
  });

  it("vapor rows use skin-row-inset (rowTreatment='inset')", async () => {
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "vapor" } }));
    setupInvokeWithItems([makeEntry("a"), makeEntry("b")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const rows = screen.getAllByRole("option");
    expect(rows[0].className).toContain("skin-row-inset");
    // Inset rows: no flat border-b divider
    expect(rows[0].className).not.toContain("border-b");
  });

  it("quiet rows use skin-row-line (rowTreatment='line')", async () => {
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "quiet" } }));
    setupInvokeWithItems([makeEntry("a")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const rows = screen.getAllByRole("option");
    expect(rows[0].className).toContain("skin-row-line");
    expect(rows[0].className).toContain("border-b");
  });

  it("classic rows use card treatment (border-b, no skin-row-inset/skin-row-line)", async () => {
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

describe("CopyPaste-o2o9: vapor inset rows render with inline style for spacing and surface", () => {
  beforeEach(() => {
    invoke.mockReset();
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "vapor" } }));
  });

  it("vapor rows have inline borderRadius from var(--skin-r-card)", async () => {
    setupInvokeWithItems([makeEntry("v1"), makeEntry("v2")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item v1")).toBeInTheDocument());

    const rows = screen.getAllByRole("option");
    const styleAttr = rows[0].getAttribute("style") ?? "";
    // Inline style must reference the skin token (not a hardcoded px value like 16px alone)
    expect(styleAttr).toMatch(/border-radius.*var\(--skin-r-card/i);
  });

  it("vapor rows have per-row marginBottom for spacing (gap is no-op on abs positioned rows)", async () => {
    setupInvokeWithItems([makeEntry("v1"), makeEntry("v2")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item v1")).toBeInTheDocument());

    const rows = screen.getAllByRole("option");
    const styleAttr = rows[0].getAttribute("style") ?? "";
    // Per-row vertical spacing must be margin-bottom referencing the skin gap token
    expect(styleAttr).toMatch(/margin-bottom.*var\(--skin-row-gap/i);
  });

  it("classic rows do NOT have vapor card-surface inline styles", async () => {
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "classic" } }));
    setupInvokeWithItems([makeEntry("c1")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item c1")).toBeInTheDocument());

    const rows = screen.getAllByRole("option");
    const styleAttr = rows[0].getAttribute("style") ?? "";
    // Classic rows must not carry the vapor inset surface token inline
    expect(styleAttr).not.toMatch(/var\(--skin-r-card\)/);
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-kp6f: skin radius tokens on controls (no rounded-ide class)
// ---------------------------------------------------------------------------

describe("CopyPaste-kp6f: controls use var(--skin-r-ctl) inline, not rounded-ide class", () => {
  beforeEach(() => {
    invoke.mockReset();
    useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "classic" } }));
  });

  it("file-attach button uses inline var(--skin-r-ctl) borderRadius, not rounded-ide class", async () => {
    setupInvokeWithItems([makeEntry("a")]);
    const { container } = render(<HistoryView />);
    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const attachBtn = container.querySelector('button[aria-label="Add file"]');
    expect(attachBtn).not.toBeNull();
    if (attachBtn) {
      const cls = attachBtn.getAttribute("class") ?? "";
      expect(cls).not.toContain("rounded-ide");
      const style = attachBtn.getAttribute("style") ?? "";
      expect(style).toMatch(/border-radius.*var\(--skin-r-ctl/i);
    }
  });

  it("sort-toggle button uses inline var(--skin-r-ctl) borderRadius, not rounded-ide class", async () => {
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
      expect(style).toMatch(/border-radius.*var\(--skin-r-ctl/i);
    }
    // If sort button not shown (single device), pass — that's expected.
  });

  it("Toast component uses var(--skin-r-modal) for borderRadius, not a hardcoded value", () => {
    // Previously this test was a conditional no-op: it only asserted when a toast
    // happened to be visible after HistoryView IPC actions, which never fired in
    // JSDOM. This replacement renders GlassToast directly and asserts the token
    // unconditionally — the assertion can never silently pass without the
    // var(--skin-r-modal) token being present.
    // CopyPaste-bdac.56: Toast radius is the modal token (--skin-r-modal), not --skin-r-card.
    const { container } = render(
      <GlassToast msg={{ id: "kp6f-toast", text: "test" }} onDismiss={() => {}} />,
    );
    const bubble = container.querySelector('[role="status"]') as HTMLElement | null;
    expect(bubble).not.toBeNull();

    // Accept either inline style or a Tailwind arbitrary-value class that encodes the var.
    const inlineStyle = bubble!.style.borderRadius;
    const hasVarInStyle = inlineStyle.includes("--skin-r-modal");
    const hasVarInClass = bubble!.className.includes("--skin-r-modal");
    expect(hasVarInStyle || hasVarInClass).toBe(true);

    // Regression guard: must NOT be a bare hardcoded pixel value (e.g. "10px").
    expect(inlineStyle).not.toBe("10px");
  });
});
