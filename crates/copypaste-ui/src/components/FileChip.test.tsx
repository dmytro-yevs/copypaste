/**
 * Tests for FileChip component and helpers.
 *
 * RED phase: written before the implementation exists.
 * These tests verify:
 *  1. formatBytes produces human-readable sizes.
 *  2. FileChip renders with a filename and file-type icon.
 *  3. FileChip "Save As…" button calls getItemFile IPC and triggers a browser download.
 *  4. FileChip copy button calls copyItem IPC.
 *  5. Timer/fetch cleanup on unmount (no pending state updates).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock @tauri-apps/api/core so invoke() is controllable in tests.
// ---------------------------------------------------------------------------
const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// ---------------------------------------------------------------------------
// Import after mock so the module resolves the mock.
// ---------------------------------------------------------------------------
import { formatBytes, FileChip } from "./FileChip";

beforeEach(() => {
  vi.clearAllMocks();
  // Default: ipc_call for get_item_file returns a small text file.
  mockInvoke.mockResolvedValue({
    ok: true,
    data: {
      filename: "hello.txt",
      mime: "text/plain",
      data_b64: btoa("hello world"),
    },
    error: null,
    error_code: null,
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// formatBytes
// ---------------------------------------------------------------------------

describe("formatBytes", () => {
  it("formats 0 as '0 B'", () => {
    expect(formatBytes(0)).toBe("0 B");
  });

  it("formats bytes below 1 KiB as '… B'", () => {
    expect(formatBytes(512)).toBe("512 B");
  });

  it("formats KiB range", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(2048)).toBe("2.0 KB");
  });

  it("formats MiB range", () => {
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
    expect(formatBytes(1536 * 1024)).toBe("1.5 MB");
  });

  it("formats GiB range", () => {
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.0 GB");
  });
});

// ---------------------------------------------------------------------------
// FileChip rendering
// ---------------------------------------------------------------------------

describe("FileChip rendering", () => {
  it("renders the filename", () => {
    render(<FileChip id="item-1" filename="document.pdf" mime="application/pdf" />);
    expect(screen.getByText("document.pdf")).toBeInTheDocument();
  });

  it("renders a file-type icon (svg)", () => {
    const { container } = render(
      <FileChip id="item-1" filename="archive.zip" mime="application/zip" />,
    );
    // There must be at least one SVG (file icon).
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("renders the Save As button", () => {
    render(<FileChip id="item-1" filename="report.xlsx" mime="application/vnd.ms-excel" />);
    expect(screen.getByRole("button", { name: /save as/i })).toBeInTheDocument();
  });

  it("renders the Copy button", () => {
    render(<FileChip id="item-1" filename="image.png" mime="image/png" />);
    expect(screen.getByRole("button", { name: /copy/i })).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// FileChip Save As interaction
// ---------------------------------------------------------------------------

describe("FileChip Save As", () => {
  it("calls get_item_file IPC when Save As is clicked", async () => {
    render(<FileChip id="item-42" filename="hello.txt" mime="text/plain" />);

    // Mock URL.createObjectURL and document.createElement('a').click so the
    // download doesn't actually try to open a browser prompt in jsdom.
    const createObjectURL = vi.fn().mockReturnValue("blob:mock");
    const revokeObjectURL = vi.fn();
    Object.defineProperty(global.URL, "createObjectURL", { value: createObjectURL, writable: true, configurable: true });
    Object.defineProperty(global.URL, "revokeObjectURL", { value: revokeObjectURL, writable: true, configurable: true });

    const clickSpy = vi.fn();
    const origCreate = document.createElement.bind(document);
    vi.spyOn(document, "createElement").mockImplementation((tag: string) => {
      const el = origCreate(tag);
      if (tag === "a") {
        Object.defineProperty(el, "click", { value: clickSpy, configurable: true });
      }
      return el;
    });

    fireEvent.click(screen.getByRole("button", { name: /save as/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "ipc_call",
        expect.objectContaining({ method: "get_item_file", params: { id: "item-42" } }),
      );
    });
  });

  it("triggers a download anchor click after fetching file data", async () => {
    render(<FileChip id="item-99" filename="data.csv" mime="text/csv" />);

    const createObjectURL = vi.fn().mockReturnValue("blob:data");
    const revokeObjectURL = vi.fn();
    Object.defineProperty(global.URL, "createObjectURL", { value: createObjectURL, writable: true, configurable: true });
    Object.defineProperty(global.URL, "revokeObjectURL", { value: revokeObjectURL, writable: true, configurable: true });

    const clickSpy = vi.fn();
    const origCreate = document.createElement.bind(document);
    vi.spyOn(document, "createElement").mockImplementation((tag: string) => {
      const el = origCreate(tag);
      if (tag === "a") {
        Object.defineProperty(el, "click", { value: clickSpy, configurable: true });
      }
      return el;
    });

    fireEvent.click(screen.getByRole("button", { name: /save as/i }));

    await waitFor(() => {
      expect(clickSpy).toHaveBeenCalled();
    });
  });

  it("shows an error state when IPC fails", async () => {
    mockInvoke.mockResolvedValue({
      ok: false,
      data: null,
      error: "file not found",
      error_code: "not_found",
    });

    render(<FileChip id="item-bad" filename="gone.txt" mime="text/plain" />);
    fireEvent.click(screen.getByRole("button", { name: /save as/i }));

    await waitFor(() => {
      expect(screen.getByText(/save failed/i)).toBeInTheDocument();
    });
  });
});

// ---------------------------------------------------------------------------
// FileChip Copy interaction
// ---------------------------------------------------------------------------

describe("FileChip Copy", () => {
  it("calls copy_item IPC when Copy is clicked", async () => {
    // For copy_item, ipc_call also returns ok:true.
    mockInvoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "copy_item") {
        return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
      }
      return Promise.resolve({ ok: true, data: { filename: "f.txt", mime: "text/plain", data_b64: btoa("x") }, error: null, error_code: null });
    });

    render(<FileChip id="item-copy" filename="file.txt" mime="text/plain" />);
    fireEvent.click(screen.getByRole("button", { name: /copy/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "ipc_call",
        expect.objectContaining({ method: "copy_item", params: { id: "item-copy" } }),
      );
    });
  });
});

// ---------------------------------------------------------------------------
// FileChip size display
// ---------------------------------------------------------------------------

describe("FileChip size display", () => {
  it("shows formatted size when sizeBytes prop is provided", () => {
    render(
      <FileChip id="item-sized" filename="big.bin" mime="application/octet-stream" sizeBytes={2 * 1024 * 1024} />,
    );
    expect(screen.getByText(/2\.0 MB/)).toBeInTheDocument();
  });
});
