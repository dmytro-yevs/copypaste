/**
 * CopyPaste-crh3.59 — QR privacy-blur regression test for crh3.21.
 *
 * Verifies that regenerating a pairing QR always re-blurs the code regardless
 * of the current reveal state.  A new PAKE session token is a fresh credential
 * that must not be visible without re-confirmation (spec §10).
 *
 * Test cases:
 *   1. reveal → regenerate → qrBlur resets to "blurred"    (the crh3.21 bug path)
 *   2. blurred → regenerate → qrBlur stays "blurred"       (regression guard)
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// IPC stubs — pairingQrSvg never resolves by default so the hook stays in
// "loading" state and the blur/reveal state changes are isolated.
// ---------------------------------------------------------------------------

const pairingQrSvg = vi.fn();

vi.mock("../lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/ipc")>();
  return {
    ...actual,
    pairingQrSvg: (...a: unknown[]) => pairingQrSvg(...a),
  };
});

import { useQrCode } from "./DevicesView/hooks/useQrCode";

beforeEach(() => {
  // Default: pairingQrSvg hangs so the hook stays in "loading" — blur state
  // changes are testable without waiting on async QR generation.
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {}));
});

// ---------------------------------------------------------------------------
// crh3.21 regression: reveal then regenerate must re-blur
// ---------------------------------------------------------------------------

describe("CopyPaste-crh3.21 regression: QR privacy-blur on regeneration", () => {
  it("re-blurs after reveal → regenerate", () => {
    const { result } = renderHook(() => useQrCode());

    // Default state: blurred.
    expect(result.current.qrBlur).toBe("blurred");

    // User explicitly reveals the QR.
    act(() => {
      result.current.handleQrReveal();
    });
    expect(result.current.qrBlur).toBe("revealed");

    // User regenerates the QR (e.g. clicks "Regenerate" while revealed).
    act(() => {
      result.current.handleQrRegenerate();
    });

    // The blur MUST be reset to "blurred" — the new QR is a fresh credential.
    expect(result.current.qrBlur).toBe("blurred");
  });

  it("stays blurred when regenerating without revealing first", () => {
    const { result } = renderHook(() => useQrCode());

    // Start blurred.
    expect(result.current.qrBlur).toBe("blurred");

    // Regenerate without revealing first.
    act(() => {
      result.current.handleQrRegenerate();
    });

    // Must remain blurred — regeneration must never expose a fresh QR automatically.
    expect(result.current.qrBlur).toBe("blurred");
  });
});
