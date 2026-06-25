/**
 * useSensitiveReveal — unit tests.
 *
 * Tests that:
 * 1. The hook returns revealed=false initially.
 * 2. setRevealed(true) exposes the item.
 * 3. A window 'blur' event re-hides the content (SCRH-7 security feature) when
 *    is_sensitive=true and maskSensitive=true.
 * 4. The blur listener is NOT attached (no re-hide) when is_sensitive=false or
 *    maskSensitive=false.
 */
import { describe, expect, it, vi, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useSensitiveReveal } from "./useSensitiveReveal";

// jsdom-based environment — window events work.
afterEach(() => {
  vi.clearAllMocks();
});

describe("useSensitiveReveal", () => {
  it("starts with revealed=false", () => {
    const { result } = renderHook(() =>
      useSensitiveReveal({ isSensitive: true, maskSensitive: true })
    );
    expect(result.current.revealed).toBe(false);
  });

  it("setRevealed(true) updates revealed to true", () => {
    const { result } = renderHook(() =>
      useSensitiveReveal({ isSensitive: true, maskSensitive: true })
    );
    act(() => {
      result.current.setRevealed(true);
    });
    expect(result.current.revealed).toBe(true);
  });

  it("window blur re-hides content when isSensitive=true and maskSensitive=true (SCRH-7)", () => {
    const { result } = renderHook(() =>
      useSensitiveReveal({ isSensitive: true, maskSensitive: true })
    );
    // Reveal the item first.
    act(() => {
      result.current.setRevealed(true);
    });
    expect(result.current.revealed).toBe(true);

    // Simulate window losing focus.
    act(() => {
      window.dispatchEvent(new Event("blur"));
    });
    expect(result.current.revealed).toBe(false);
  });

  it("window blur does NOT re-hide when isSensitive=false", () => {
    const { result } = renderHook(() =>
      useSensitiveReveal({ isSensitive: false, maskSensitive: true })
    );
    act(() => {
      result.current.setRevealed(true);
    });
    act(() => {
      window.dispatchEvent(new Event("blur"));
    });
    // isSensitive=false → no blur listener → still revealed
    expect(result.current.revealed).toBe(true);
  });

  it("window blur does NOT re-hide when maskSensitive=false", () => {
    const { result } = renderHook(() =>
      useSensitiveReveal({ isSensitive: true, maskSensitive: false })
    );
    act(() => {
      result.current.setRevealed(true);
    });
    act(() => {
      window.dispatchEvent(new Event("blur"));
    });
    // maskSensitive=false → no blur listener → still revealed
    expect(result.current.revealed).toBe(true);
  });

  it("removes the blur listener on unmount", () => {
    const addSpy = vi.spyOn(window, "addEventListener");
    const removeSpy = vi.spyOn(window, "removeEventListener");

    const { unmount } = renderHook(() =>
      useSensitiveReveal({ isSensitive: true, maskSensitive: true })
    );

    // Listener should have been registered.
    const addedBlurCalls = addSpy.mock.calls.filter((c) => c[0] === "blur");
    expect(addedBlurCalls.length).toBeGreaterThan(0);

    unmount();

    // Listener should have been removed.
    const removedBlurCalls = removeSpy.mock.calls.filter((c) => c[0] === "blur");
    expect(removedBlurCalls.length).toBeGreaterThan(0);
  });
});
