/**
 * Tests for CopyPaste-cg2h: truncated + tap-to-copy fingerprint on ThisDeviceCard.
 *
 * PG-9 spec: own-device fingerprint shown as truncated (first 8 + "…" + last 8)
 * with a tap-to-copy interaction (Android parity style).
 *
 * Verifies:
 *  1. A full 64-char fingerprint is truncated in display (not shown in full).
 *  2. The truncated form shows the first 8 chars.
 *  3. The truncated form shows the last 8 chars.
 *  4. A copy button / clickable element is present.
 *  5. Clicking copies the FULL fingerprint to the clipboard.
 *  6. Null fingerprint (P2P disabled) renders nothing.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent } from "@testing-library/react";
import { ThisDeviceCard } from "./DeviceCard";
import type { OwnDeviceInfo } from "../lib/ipc";

const MOCK_FINGERPRINT =
  "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2";

const MOCK_INFO: OwnDeviceInfo = {
  fingerprint: MOCK_FINGERPRINT,
  device_name: "Test Mac",
  device_model: "MacBook Pro",
  os_version: "macOS 15.0",
  app_version: "0.7.4",
  local_ip: "192.168.1.1",
  public_ip: null,
};

describe("ThisDeviceCard: truncated fingerprint (CopyPaste-cg2h)", () => {
  let originalClipboard: Clipboard;

  beforeEach(() => {
    originalClipboard = navigator.clipboard;
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
      writable: true,
      configurable: true,
    });
  });

  afterEach(() => {
    Object.defineProperty(navigator, "clipboard", {
      value: originalClipboard,
      writable: true,
      configurable: true,
    });
  });

  it("does NOT show the full 64-char fingerprint as plain text", () => {
    const { container } = render(<ThisDeviceCard info={MOCK_INFO} />);
    // The full fingerprint must NOT appear verbatim in any text node
    expect(container.textContent).not.toContain(MOCK_FINGERPRINT);
  });

  it("shows the first 8 chars of the fingerprint", () => {
    const { container } = render(<ThisDeviceCard info={MOCK_INFO} />);
    expect(container.textContent).toContain(MOCK_FINGERPRINT.slice(0, 8));
  });

  it("shows the last 8 chars of the fingerprint", () => {
    const { container } = render(<ThisDeviceCard info={MOCK_INFO} />);
    expect(container.textContent).toContain(MOCK_FINGERPRINT.slice(-8));
  });

  it("has a clickable element with title or aria-label about copying the fingerprint", () => {
    const { container } = render(<ThisDeviceCard info={MOCK_INFO} />);
    // The copy target must have a title or button role indicating copy intent
    const copyEl =
      container.querySelector("[data-testid='fingerprint-copy']") ??
      container.querySelector("[title*='fingerprint']") ??
      container.querySelector("[title*='Fingerprint']") ??
      container.querySelector("[aria-label*='fingerprint']") ??
      container.querySelector("[aria-label*='Fingerprint']") ??
      container.querySelector("button[class*='fingerprint']");
    expect(copyEl).not.toBeNull();
  });

  it("clicking the fingerprint row copies the full fingerprint to clipboard", async () => {
    const { container } = render(<ThisDeviceCard info={MOCK_INFO} />);
    const copyEl =
      container.querySelector("[data-testid='fingerprint-copy']") ??
      container.querySelector("[title*='fingerprint']") ??
      container.querySelector("[title*='Fingerprint']") ??
      container.querySelector("[aria-label*='fingerprint']") ??
      container.querySelector("[aria-label*='Fingerprint']");
    expect(copyEl).not.toBeNull();
    fireEvent.click(copyEl!);
    // Allow any pending promises to flush
    await Promise.resolve();
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(MOCK_FINGERPRINT);
  });

  it("null fingerprint renders nothing for fingerprint row", () => {
    const infoNoFp: OwnDeviceInfo = { ...MOCK_INFO, fingerprint: null };
    const { container } = render(<ThisDeviceCard info={infoNoFp} />);
    // No fingerprint label or value should appear
    expect(container.textContent).not.toContain("Fingerprint");
  });
});
