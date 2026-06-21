/**
 * Security tests for bd CopyPaste-1jms.1, 1jms.5, 1jms.3, 1jms.12.
 *
 * 1jms.1: SAS verification code must be non-copyable (userSelect: none, no
 *         click-to-copy path that writes to the clipboard).
 * 1jms.5: QR payload text must be non-selectable (userSelect: none).
 * 1jms.3: pairAbort/pairReset is called after ABORT terminal state.
 * 1jms.12: pairAbort/pairReset is called after CONFIRM terminal state.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, waitFor } from "@testing-library/react";
import type { PairSasStatus } from "../lib/ipc";

// ---------------------------------------------------------------------------
// IPC stubs
// ---------------------------------------------------------------------------

const getOwnDeviceInfo = vi.fn();
const listPeers = vi.fn();
const probeStatus = vi.fn();
const pairingQrSvg = vi.fn();
const pairGetSas = vi.fn();
const pairAbort = vi.fn();
const pairConfirmSas = vi.fn();
const listDiscovered = vi.fn();

vi.mock("../lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/ipc")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      getOwnDeviceInfo: (...a: unknown[]) => getOwnDeviceInfo(...a),
      listPeers: (...a: unknown[]) => listPeers(...a),
      listDiscovered: (...a: unknown[]) => listDiscovered(...a),
      revokeAllPeers: vi.fn().mockResolvedValue({ revoked: 0 }),
      revokePeer: vi.fn().mockResolvedValue({ revoked_at: "2024-01-01" }),
      unpairPeer: vi.fn().mockResolvedValue(undefined),
      pairGetSas: (...a: unknown[]) => pairGetSas(...a),
      pairAbort: (...a: unknown[]) => pairAbort(...a),
      pairConfirmSas: (...a: unknown[]) => pairConfirmSas(...a),
      revokeAndRotate: vi.fn().mockResolvedValue({ revoked_at: "2024-01-01" }),
    },
    probeStatus: (...a: unknown[]) => probeStatus(...a),
    pairingQrSvg: (...a: unknown[]) => pairingQrSvg(...a),
  };
});

import { DevicesView } from "./DevicesView";

// ---------------------------------------------------------------------------
// Common setup
// ---------------------------------------------------------------------------

beforeEach(() => {
  getOwnDeviceInfo.mockReset().mockResolvedValue({
    fingerprint: "AB12CD34EF56",
    device_name: "Test Mac",
    device_model: "MacBook Air",
    os_version: "macOS 15.5",
    app_version: "0.6.0",
    local_ip: null,
  });
  listPeers.mockReset().mockResolvedValue({ peers: [] });
  listDiscovered.mockReset().mockResolvedValue({ devices: [] });
  probeStatus.mockReset().mockResolvedValue({ kind: "ready" });
  // QR that never resolves so the QR section stays in loading state by default.
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {}));
  pairGetSas.mockReset().mockResolvedValue({ state: "awaiting_sas", sas: "123456", role: "responder" });
  pairAbort.mockReset().mockResolvedValue({ ok: true });
  pairConfirmSas.mockReset().mockResolvedValue({ ok: true, accepted: true });
});

afterEach(() => {
  vi.useRealTimers();
});

// ---------------------------------------------------------------------------
// CopyPaste-1jms.1: SAS code is non-copyable
// ---------------------------------------------------------------------------

describe("CopyPaste-1jms.1: SAS code is display-only (non-copyable)", () => {
  it("renders the SAS code with userSelect:none style", async () => {
    const incoming: PairSasStatus = {
      state: "awaiting_sas",
      sas: "123456",
      role: "responder",
    };

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    // The SAS code display element must be present.
    const sasDisplay = await screen.findByTestId("sas-code-display");
    expect(sasDisplay).toBeInTheDocument();
    expect(sasDisplay.textContent).toContain("123456");

    // Must have userSelect: none so the code cannot be selected/copied.
    const style = sasDisplay.getAttribute("style") ?? "";
    expect(style).toMatch(/user-select\s*:\s*none/i);
  });

  it("does not render a click-to-copy button for the SAS code", async () => {
    const incoming: PairSasStatus = {
      state: "awaiting_sas",
      sas: "654321",
      role: "responder",
    };

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    // The old "Click to copy" button must not exist.
    await screen.findByTestId("sas-code-display"); // wait for SAS to render
    const copyButton = screen.queryByTitle(/click to copy/i);
    expect(copyButton).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-1jms.5: QR payload text is non-selectable
// ---------------------------------------------------------------------------

describe("CopyPaste-1jms.5: QR payload NEVER rendered in the DOM", () => {
  // The raw CPPAIR2.* payload (PAKE password, device cert fingerprint, Supabase
  // anon key) must not appear in the DOM even when the QR is revealed.
  // userSelect:none was insufficient because element.textContent / browser
  // extensions / execCommand('copy') could still extract the secret.
  // Fix: the <p> block is removed entirely — the QR SVG is the only channel.

  it("does not render the QR payload text in the DOM while the QR is blurred", async () => {
    pairingQrSvg.mockReset().mockResolvedValue({
      svg: "<svg><rect/></svg>",
      payload: "CPPAIR2.secret.pake.payload.string",
      expires_in_secs: 120,
    });
    pairGetSas.mockReset().mockResolvedValue({ state: "idle" });

    await act(async () => {
      render(<DevicesView />);
    });

    // Wait for QR to load (reveal button should be visible)
    await screen.findByRole("button", { name: /click to reveal/i });

    // Payload must not appear anywhere in the DOM while blurred.
    expect(
      document.body.textContent
    ).not.toContain("CPPAIR2.secret.pake.payload.string");

    // The [data-testid="qr-payload-text"] element must not exist.
    expect(screen.queryByTestId("qr-payload-text")).not.toBeInTheDocument();
  });

  it("does not render the QR payload text in the DOM after revealing", async () => {
    pairingQrSvg.mockReset().mockResolvedValue({
      svg: "<svg><rect/></svg>",
      payload: "CPPAIR2.secret.pake.payload.string",
      expires_in_secs: 120,
    });
    pairGetSas.mockReset().mockResolvedValue({ state: "idle" });

    await act(async () => {
      render(<DevicesView />);
    });

    // Reveal the QR: find and click "Click to reveal".
    const revealBtn = await screen.findByRole("button", { name: /click to reveal/i });
    await act(async () => {
      revealBtn.click();
    });

    // After reveal, the [data-testid="qr-payload-text"] element must NOT exist.
    // The raw payload must not be in any DOM text node.
    expect(screen.queryByTestId("qr-payload-text")).not.toBeInTheDocument();
    expect(
      document.body.textContent
    ).not.toContain("CPPAIR2.secret.pake.payload.string");
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-1jms.3: pairAbort called after ABORT terminal state
// ---------------------------------------------------------------------------

describe("CopyPaste-1jms.3: pairAbort is called after ABORT terminal close", () => {
  it("calls pairAbort when the user closes a modal in aborted state", async () => {
    const incoming: PairSasStatus = {
      state: "aborted",
      role: "responder",
    };

    // Seed with aborted state directly so the modal renders terminal.
    pairGetSas.mockReset().mockResolvedValue({ state: "aborted" });

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    // The SAS modal must be open in aborted (terminal) state.
    const dialog = await screen.findByRole("dialog", { name: /wants to pair|Pair "/i });
    expect(dialog).toBeInTheDocument();

    // Find the "Close" button on the terminal failure panel.
    const closeBtn = await screen.findByRole("button", { name: /^Close$/i });
    pairAbort.mockClear();

    await act(async () => {
      closeBtn.click();
    });

    // pairAbort must have been called to reset the state machine.
    await waitFor(() => {
      expect(pairAbort).toHaveBeenCalled();
    });
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-1jms.12: pairAbort called after CONFIRM terminal state
// ---------------------------------------------------------------------------

describe("CopyPaste-1jms.12: pairAbort is called after CONFIRM terminal close", () => {
  it("calls pairAbort when the user closes a modal in confirmed state", async () => {
    const incoming: PairSasStatus = {
      state: "confirmed",
      role: "responder",
    };

    // Poll returns confirmed immediately.
    pairGetSas.mockReset().mockResolvedValue({ state: "confirmed" });

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    // The SAS modal must be open in confirmed (terminal success) state.
    const dialog = await screen.findByRole("dialog", { name: /wants to pair|Pair "/i });
    expect(dialog).toBeInTheDocument();

    // Find the "Close" button on the terminal success panel.
    const closeBtn = await screen.findByRole("button", { name: /^Close$/i });
    pairAbort.mockClear();

    await act(async () => {
      closeBtn.click();
    });

    // pairAbort must have been called to reset the state machine.
    await waitFor(() => {
      expect(pairAbort).toHaveBeenCalled();
    });
  });
});
