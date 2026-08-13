/**
 * DMY-136: `useCaptureState`'s startup read used to inherit `ipcCall`'s five
 * minute default timeout with no retry, so a native command that never
 * answered left Android navigation disabled for five minutes — past the
 * release gate's 240 s observation window. These tests exercise the real
 * `ipcCall` boundary (only the Tauri `invoke` is mocked) so the bound timeout
 * and the causal, error-driven retry are both actually under test rather than
 * short-circuited by a module-level mock of `captureState` itself.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { useCaptureState } from "@/hooks/useCapture";
import { testClient } from "@/test/harness";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

interface Deferred<T> {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
  readonly reject: (reason: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((onResolve, onReject) => {
    resolve = onResolve;
    reject = onReject;
  });
  return { promise, resolve, reject };
}

function wrapper() {
  const client = testClient();
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

beforeEach(() => {
  invoke.mockReset();
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

describe("startup capture-state read", () => {
  it("does not block forever on a native command that never answers", async () => {
    vi.useFakeTimers();
    invoke.mockReturnValue(new Promise(() => {}));

    const { result } = renderHook(() => useCaptureState(), { wrapper: wrapper() });

    expect(result.current.isPending).toBe(true);

    // Six bounded attempts (one initial plus five retries) at the 10 s
    // startup bound, each followed by react-query's backoff delay — well
    // under the five-minute `ipcCall` default this replaces.
    await vi.advanceTimersByTimeAsync(5 * 60_000);

    expect(result.current.isError).toBe(true);
    expect(result.current.isPending).toBe(false);
  });

  it("retries a transient failure and recovers", async () => {
    vi.useFakeTimers();
    invoke
      .mockRejectedValueOnce({ code: "offline", retryable: true })
      .mockRejectedValueOnce({ code: "offline", retryable: true })
      .mockResolvedValue({ health: { state: "working" } });

    const { result } = renderHook(() => useCaptureState(), { wrapper: wrapper() });

    await vi.advanceTimersByTimeAsync(10_000);

    expect(result.current.isSuccess).toBe(true);
    expect(result.current.data).toEqual({ health: { state: "working" } });
    expect(invoke).toHaveBeenCalledTimes(3);
  });

  it("never retries a permanent failure", async () => {
    invoke.mockRejectedValue({ code: "auth_failed", retryable: false });

    const { result } = renderHook(() => useCaptureState(), { wrapper: wrapper() });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("bounds each startup attempt to a short timeout, not ipcCall's default", async () => {
    vi.useFakeTimers();
    const native = deferred<never>();
    invoke.mockReturnValue(native.promise);

    const { result } = renderHook(() => useCaptureState(), { wrapper: wrapper() });

    // The first bounded attempt gives up well short of `ipcCall`'s five
    // minute default, freeing react-query to schedule a retry.
    await vi.advanceTimersByTimeAsync(10_000);
    expect(invoke).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(10_000);
    expect(invoke.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(result.current.isPending).toBe(true);
  });
});
