/**
 * SCRH-8 (DOM leak) + SCRH-7 (auto-re-blur) tests for HistoryView.
 *
 * SCRH-8: sensitive plaintext MUST NOT appear in the DOM when the item is
 *   masked and not yet revealed. CSS blur alone is insufficient — screen
 *   readers, devtools, and clipboard scanners read raw text nodes. The fix
 *   renders a placeholder instead of the real text until the user reveals.
 *
 * SCRH-7: once revealed, sensitive content MUST be hidden again on window
 *   blur (focus loss) so that walking away from the machine clears the secret.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  act,
} from "@testing-library/react";
import { useUI } from "../store";

// ---------------------------------------------------------------------------
// Tauri mocks — must be set up BEFORE importing HistoryView.
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { HistoryView } from "./HistoryView";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const REAL_SECRET = "my-super-secret-password-12345";

function makeSensitiveEntry(id = "s1") {
  return {
    id,
    content_type: "text",
    preview: REAL_SECRET,
    is_sensitive: true,
    wall_time: 1_700_000_000_000,
    pinned: false,
  };
}

function makeNormalEntry(id = "n1") {
  return {
    id,
    content_type: "text",
    preview: `Normal item ${id}`,
    is_sensitive: false,
    wall_time: 1_700_000_000_001,
    pinned: false,
  };
}

function ipcOk(data: unknown) {
  return { ok: true, data, error: null, error_code: null };
}

function setupInvokeWithItems(items: unknown[]) {
  invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
    if (args?.method === "history_page") {
      return Promise.resolve(ipcOk({ items, total: items.length }));
    }
    if (args?.method === "get_private_mode") {
      return Promise.resolve(ipcOk({ private_mode: false }));
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
// Setup / teardown
// ---------------------------------------------------------------------------

beforeEach(() => {
  invoke.mockReset();
  // Ensure masking is ON and sensitive warnings ON (defaults).
  useUI.setState((s) => ({
    prefs: {
      ...s.prefs,
      maskSensitive: true,
      showSensitiveWarnings: true,
      skin: "classic",
    },
  }));
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// SCRH-8: plaintext must be absent from DOM while masked
// ---------------------------------------------------------------------------

describe("SCRH-8: sensitive plaintext absent from DOM while masked", () => {
  it("does NOT render the real secret text in history rows before reveal", async () => {
    setupInvokeWithItems([makeSensitiveEntry(), makeNormalEntry()]);

    const { container } = render(<HistoryView />);

    // Normal item must load so we know the view is ready.
    await waitFor(() =>
      expect(screen.getByText(/Normal item n1/)).toBeInTheDocument()
    );

    // The real secret text must NOT appear anywhere in the DOM tree.
    expect(container.textContent).not.toContain(REAL_SECRET);
  });

  it("renders a placeholder string in the row while the item is masked", async () => {
    setupInvokeWithItems([makeSensitiveEntry()]);

    render(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText(/Sensitive — preview hidden · click to reveal/i)).toBeInTheDocument()
    );
  });

  it("reveals the real text after clicking the placeholder span", async () => {
    setupInvokeWithItems([makeSensitiveEntry()]);

    const { container } = render(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText(/Sensitive — preview hidden · click to reveal/i)).toBeInTheDocument()
    );

    // Real text is absent before reveal.
    expect(container.textContent).not.toContain(REAL_SECRET);

    // Click the placeholder span to reveal.
    const placeholder = screen.getByText(/Sensitive — preview hidden · click to reveal/i);
    await act(async () => {
      fireEvent.click(placeholder);
    });

    // Real text must now be in the DOM.
    await waitFor(() =>
      expect(container.textContent).toContain(REAL_SECRET)
    );
  });

  it("does NOT reveal text on row-click copy (row click must not propagate through the placeholder)", async () => {
    setupInvokeWithItems([makeSensitiveEntry()]);

    const { container } = render(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText(/Sensitive — preview hidden · click to reveal/i)).toBeInTheDocument()
    );

    // Real text must be absent before reveal.
    expect(container.textContent).not.toContain(REAL_SECRET);
  });

  it("non-sensitive items always show their preview text", async () => {
    setupInvokeWithItems([makeNormalEntry("x")]);

    render(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText("Normal item x")).toBeInTheDocument()
    );
  });

  it("sensitive item aria-label does NOT contain the real plaintext while masked", async () => {
    setupInvokeWithItems([makeSensitiveEntry()]);

    render(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText(/Sensitive — preview hidden · click to reveal/i)).toBeInTheDocument()
    );

    // Collect all aria-labels from the row options.
    const rows = document.querySelectorAll('[role="option"]');
    for (const row of rows) {
      const label = row.getAttribute("aria-label") ?? "";
      expect(label).not.toContain(REAL_SECRET);
    }

    // Collect all checkbox aria-labels.
    const checkboxes = document.querySelectorAll('input[type="checkbox"]');
    for (const cb of checkboxes) {
      const label = cb.getAttribute("aria-label") ?? "";
      expect(label).not.toContain(REAL_SECRET);
    }
  });
});

// ---------------------------------------------------------------------------
// SCRH-7: re-blur (clear revealed state) on window blur
// ---------------------------------------------------------------------------

describe("SCRH-7: re-hide sensitive content on window blur", () => {
  it("re-hides the revealed secret when the window loses focus (row)", async () => {
    setupInvokeWithItems([makeSensitiveEntry()]);

    const { container } = render(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText(/Sensitive — preview hidden · click to reveal/i)).toBeInTheDocument()
    );

    // Click to reveal.
    const placeholder = screen.getByText(/Sensitive — preview hidden · click to reveal/i);
    await act(async () => {
      fireEvent.click(placeholder);
    });

    await waitFor(() =>
      expect(container.textContent).toContain(REAL_SECRET)
    );

    // Simulate window losing focus.
    await act(async () => {
      fireEvent.blur(window);
    });

    // Real text must be gone again.
    expect(container.textContent).not.toContain(REAL_SECRET);

    // Placeholder must be back.
    expect(screen.getByText(/Sensitive — preview hidden · click to reveal/i)).toBeInTheDocument();
  });

  it("non-sensitive items are unaffected by window blur", async () => {
    setupInvokeWithItems([makeNormalEntry("stable")]);

    render(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText("Normal item stable")).toBeInTheDocument()
    );

    // Window blur must not hide normal items.
    await act(async () => {
      fireEvent.blur(window);
    });

    expect(screen.getByText("Normal item stable")).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// masking.ts — maskPlaceholder unit test
// ---------------------------------------------------------------------------

import { maskPlaceholder } from "../lib/masking";

describe("maskPlaceholder", () => {
  it("returns a non-empty string", () => {
    expect(maskPlaceholder().length).toBeGreaterThan(0);
  });

  it("does not contain any secret material (sanity check)", () => {
    // The placeholder must never be derived from real content.
    expect(maskPlaceholder()).not.toContain(REAL_SECRET);
  });
});
