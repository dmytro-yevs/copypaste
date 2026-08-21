import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { call } from "./ipcCall";

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

const originalFetch = globalThis.fetch;
const fetchMock = vi.fn<typeof fetch>();

function setTauriBridge(enabled: boolean): void {
  if (enabled) {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
  } else {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  }
}

function setWebBridge(): void {
  setTauriBridge(false);
  vi.stubEnv("DEV", true);
  vi.stubEnv("VITE_COPYPASTE_WEB_BRIDGE_URL", "http://127.0.0.1:43123");
  vi.stubEnv("VITE_COPYPASTE_WEB_BRIDGE_TOKEN", "test-token");
}

function setRuntimeWebBridge(): void {
  setTauriBridge(false);
  vi.stubEnv("DEV", true);
  window.__COPYPASTE_WEB_BRIDGE__ = {
    url: "http://127.0.0.1:43123",
    token: "test-token",
  };
}

beforeEach(() => {
  invoke.mockReset();
  fetchMock.mockReset();
  globalThis.fetch = fetchMock;
  setTauriBridge(true);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  globalThis.fetch = originalFetch;
  delete window.__COPYPASTE_WEB_BRIDGE__;
  setTauriBridge(false);
});

describe("IPC call lifecycle", () => {
  it("cleans up its timeout and cancellation listener after success", async () => {
    vi.useFakeTimers();
    invoke.mockResolvedValue({ online: true });
    const controller = new AbortController();
    const remove = vi.spyOn(controller.signal, "removeEventListener");

    await expect(call("status", undefined, {
      signal: controller.signal,
      timeoutMs: 20,
    })).resolves.toEqual({ online: true });

    expect(remove).toHaveBeenCalledWith("abort", expect.any(Function));
    expect(vi.getTimerCount()).toBe(0);
  });

  it("does not start work for an already-cancelled caller", async () => {
    const controller = new AbortController();
    controller.abort();

    await expect(call("status", undefined, {
      signal: controller.signal,
    })).rejects.toMatchObject({ code: "cancelled", retryable: false });
    expect(invoke).not.toHaveBeenCalled();
  });

  it("stops waiting for Tauri without pretending to cancel the command", async () => {
    vi.useFakeTimers();
    const native = deferred<string>();
    invoke.mockReturnValue(native.promise);
    const controller = new AbortController();

    const outcome = call<string>("status", undefined, {
      signal: controller.signal,
      timeoutMs: 20,
    });
    controller.abort();

    await expect(outcome).rejects.toMatchObject({
      code: "cancelled",
      retryable: false,
    });
    expect(invoke).toHaveBeenCalledWith("status", undefined);
    expect(vi.getTimerCount()).toBe(0);

    native.resolve("late");
    await Promise.resolve();
  });

  it("classifies a Tauri timeout and suppresses its late rejection", async () => {
    vi.useFakeTimers();
    const native = deferred<never>();
    invoke.mockReturnValue(native.promise);

    const outcome = call("status", undefined, { timeoutMs: 20 });
    const rejection = expect(outcome).rejects.toMatchObject({
      code: "timeout",
      retryable: true,
    });
    await vi.advanceTimersByTimeAsync(20);
    await rejection;
    expect(vi.getTimerCount()).toBe(0);

    native.reject({ code: "internal", retryable: true });
    await Promise.resolve();
  });

  it("bounds calls that do not supply explicit lifecycle options", async () => {
    vi.useFakeTimers();
    invoke.mockReturnValue(new Promise(() => {}));

    const outcome = call("status");
    const rejection = expect(outcome).rejects.toMatchObject({
      code: "timeout",
      retryable: true,
    });
    await vi.advanceTimersByTimeAsync(5 * 60_000);

    await rejection;
  });

  it("aborts the browser request when its caller cancels", async () => {
    vi.useFakeTimers();
    setWebBridge();
    let requestSignal: AbortSignal | null = null;
    fetchMock.mockImplementation((_input, init) => new Promise((_resolve, reject) => {
      requestSignal = init?.signal ?? null;
      requestSignal?.addEventListener("abort", () => {
        reject(new DOMException("aborted", "AbortError"));
      }, { once: true });
    }));
    const controller = new AbortController();

    const outcome = call("status", undefined, {
      signal: controller.signal,
      timeoutMs: 20,
    });
    controller.abort();

    await expect(outcome).rejects.toMatchObject({
      code: "cancelled",
      retryable: false,
    });
    expect(requestSignal).toMatchObject({ aborted: true });
    expect(vi.getTimerCount()).toBe(0);
  });

  it("aborts and classifies a browser timeout", async () => {
    vi.useFakeTimers();
    setWebBridge();
    let requestSignal: AbortSignal | null = null;
    fetchMock.mockImplementation((_input, init) => new Promise((_resolve, reject) => {
      requestSignal = init?.signal ?? null;
      requestSignal?.addEventListener("abort", () => {
        reject(new DOMException("aborted", "AbortError"));
      }, { once: true });
    }));

    const outcome = call("status", undefined, { timeoutMs: 20 });
    const rejection = expect(outcome).rejects.toMatchObject({
      code: "timeout",
      retryable: true,
    });
    await vi.advanceTimersByTimeAsync(20);
    await rejection;

    expect(requestSignal).toMatchObject({ aborted: true });
    expect(vi.getTimerCount()).toBe(0);
  });

  it("uses the same structured failure from either bridge", async () => {
    invoke.mockRejectedValue({ code: "offline", retryable: true });
    await expect(call("status")).rejects.toMatchObject({
      code: "offline",
      retryable: true,
    });

    setWebBridge();
    fetchMock.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ code: "offline", retryable: true }),
    } as Response);
    await expect(call("status")).rejects.toMatchObject({
      code: "offline",
      retryable: true,
    });
  });

  it("returns the same successful result from either bridge", async () => {
    invoke.mockResolvedValue({ item_count: 3 });
    await expect(call("status")).resolves.toEqual({ item_count: 3 });

    setWebBridge();
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ item_count: 3 }),
    } as Response);
    await expect(call("status")).resolves.toEqual({ item_count: 3 });
  });

  it("uses the runtime bridge file without restarting Vite", async () => {
    setRuntimeWebBridge();
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ item_count: 3 }),
    } as Response);

    await expect(call("status")).resolves.toEqual({ item_count: 3 });
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:43123/v1/call",
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: "Bearer test-token",
        }),
      }),
    );
  });

  it("classifies a missing or unreachable bridge as offline", async () => {
    setTauriBridge(false);
    await expect(call("status")).rejects.toMatchObject({
      code: "offline",
      retryable: true,
    });

    setWebBridge();
    fetchMock.mockRejectedValue(new TypeError("fetch failed"));
    await expect(call("status")).rejects.toMatchObject({
      code: "offline",
      retryable: true,
    });
  });
});
