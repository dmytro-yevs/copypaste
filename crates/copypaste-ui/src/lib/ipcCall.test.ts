import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { call, hasWebBridge } from "./ipcCall";
import { previewScenarioStore } from "@/service/previewScenario";

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
const TEST_BRIDGE_URL = "http://127.0.0.1:43123";
const TEST_BRIDGE_TOKEN = "0123456789abcdef0123456789abcdef";

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
  window.__COPYPASTE_WEB_BRIDGE__ = {
    url: TEST_BRIDGE_URL,
    token: TEST_BRIDGE_TOKEN,
  };
}

function setRuntimeWebBridge(): void {
  setTauriBridge(false);
  vi.stubEnv("DEV", true);
  window.__COPYPASTE_WEB_BRIDGE__ = {
    url: TEST_BRIDGE_URL,
    token: TEST_BRIDGE_TOKEN,
  };
}

function healthyBridgeResponse(): Response {
  return { ok: true, status: 204 } as Response;
}

beforeEach(() => {
  invoke.mockReset();
  fetchMock.mockReset();
  globalThis.fetch = fetchMock;
  window.sessionStorage.clear();
  previewScenarioStore.getState().resetToLive();
  setTauriBridge(true);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  globalThis.fetch = originalFetch;
  delete window.__COPYPASTE_WEB_BRIDGE__;
  previewScenarioStore.getState().resetToLive();
  window.sessionStorage.clear();
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

    const outcome = call("status", undefined, {
      signal: controller.signal,
      timeoutMs: 20,
    });
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
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
    setWebBridge();
    const requestStarted = deferred<AbortSignal>();
    fetchMock.mockImplementation((input, init) => {
      if (String(input).endsWith("/health")) return Promise.resolve(healthyBridgeResponse());
      return new Promise((_resolve, reject) => {
        const requestSignal = init?.signal;
        if (requestSignal === undefined || requestSignal === null) {
          reject(new Error("browser command request has no AbortSignal"));
          return;
        }
        requestStarted.resolve(requestSignal);
        requestSignal.addEventListener("abort", () => {
          reject(new DOMException("aborted", "AbortError"));
        }, { once: true });
      });
    });
    const controller = new AbortController();

    const outcome = call("status", undefined, {
      signal: controller.signal,
      timeoutMs: 30_000,
    });
    const requestSignal = await requestStarted.promise;
    controller.abort();

    await expect(outcome).rejects.toMatchObject({
      code: "cancelled",
      retryable: false,
    });
    expect(requestSignal).toMatchObject({ aborted: true });
  });

  it("does not issue a browser command when cancelled before it starts", async () => {
    setWebBridge();
    fetchMock.mockImplementation((input) => String(input).endsWith("/health")
      ? Promise.resolve(healthyBridgeResponse())
      : Promise.reject(new Error("browser command request must not start")));
    const controller = new AbortController();

    const outcome = call("status", undefined, { signal: controller.signal });
    controller.abort();

    await expect(outcome).rejects.toMatchObject({
      code: "cancelled",
      retryable: false,
    });
    expect(fetchMock.mock.calls.some(([input]) =>
      String(input).endsWith("/v1/call"),
    )).toBe(false);
  });

  it("aborts and classifies a browser timeout", async () => {
    vi.useFakeTimers();
    setWebBridge();
    let requestSignal: AbortSignal | null = null;
    fetchMock.mockImplementation((input, init) => {
      if (String(input).endsWith("/health")) return Promise.resolve(healthyBridgeResponse());
      return new Promise((_resolve, reject) => {
        requestSignal = init?.signal ?? null;
        requestSignal?.addEventListener("abort", () => {
          reject(new DOMException("aborted", "AbortError"));
        }, { once: true });
      });
    });

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
    fetchMock.mockImplementation((input) => String(input).endsWith("/health")
      ? Promise.resolve(healthyBridgeResponse())
      : Promise.resolve({
          ok: false,
          status: 503,
          json: () => Promise.resolve({ code: "offline", retryable: true }),
        } as Response));
    await expect(call("status")).rejects.toMatchObject({
      code: "offline",
      retryable: true,
    });
  });

  it("returns the same successful result from either bridge", async () => {
    invoke.mockResolvedValue({ item_count: 3 });
    await expect(call("status")).resolves.toEqual({ item_count: 3 });

    setWebBridge();
    fetchMock.mockImplementation((input) => String(input).endsWith("/health")
      ? Promise.resolve(healthyBridgeResponse())
      : Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve({ item_count: 3 }),
        } as Response));
    await expect(call("status")).resolves.toEqual({ item_count: 3 });
  });

  it("prefers the native bridge when a browser runtime is present", async () => {
    vi.stubEnv("DEV", true);
    window.__COPYPASTE_WEB_BRIDGE__ = {
      url: TEST_BRIDGE_URL,
      token: TEST_BRIDGE_TOKEN,
    };
    invoke.mockResolvedValue({ item_count: 3 });

    expect(hasWebBridge()).toBe(false);
    await expect(call("status")).resolves.toEqual({ item_count: 3 });
    expect(invoke).toHaveBeenCalledWith("status", undefined);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("uses the runtime bridge file without restarting Vite", async () => {
    setRuntimeWebBridge();
    fetchMock.mockImplementation((input) => String(input).endsWith("/health")
      ? Promise.resolve(healthyBridgeResponse())
      : Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve({ item_count: 3 }),
        } as Response));

    await expect(call("status")).resolves.toEqual({ item_count: 3 });
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:43123/v1/call",
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: `Bearer ${TEST_BRIDGE_TOKEN}`,
        }),
      }),
    );
  });

  it("uses the active preview store as the IPC source of truth", async () => {
    setRuntimeWebBridge();
    previewScenarioStore.getState().addDevice({
      id: "windows-preview",
      name: "Studio PC",
      type: "desktop",
      platform: "windows",
      osName: "Windows",
      osVersion: "11 24H2",
      model: "Surface Studio 2+",
      appVersion: "2.0.0-preview",
      protocolVersion: 2,
      latencyMs: 32,
      lanIp: "192.168.1.31",
      port: 49_232,
      publicIp: null,
      location: null,
      lastSeenAgeMs: 1_000,
      online: true,
      paired: false,
    });

    window.sessionStorage.clear();

    await expect(call("discovered")).resolves.toMatchObject([
      { discovery_id: "windows-preview", name: "Studio PC" },
    ]);
    await expect(call("rescan")).resolves.toMatchObject([
      { discovery_id: "windows-preview", details: { latency: {
        connect_latency_ms: 32,
      } } },
    ]);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("returns to the real bridge only after Reset to Live", async () => {
    setRuntimeWebBridge();
    previewScenarioStore.getState().setResource("discovery", "empty");
    fetchMock.mockImplementation((input) => String(input).endsWith("/health")
      ? Promise.resolve(healthyBridgeResponse())
      : Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve([{ discovery_id: "live-device" }]),
        } as Response));

    await expect(call("rescan")).resolves.toEqual([]);
    expect(fetchMock).not.toHaveBeenCalled();

    previewScenarioStore.getState().resetToLive();

    await expect(call("rescan")).resolves.toEqual([
      { discovery_id: "live-device" },
    ]);
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:43123/v1/call",
      expect.objectContaining({
        body: JSON.stringify({ command: "rescan", args: {} }),
      }),
    );
  });

  it("fails fast when the published runtime is stale", async () => {
    setRuntimeWebBridge();
    fetchMock.mockRejectedValue(new TypeError("fetch failed"));
    const append = vi.spyOn(document.head, "appendChild");

    await expect(call("status")).rejects.toMatchObject({
      code: "offline",
      retryable: true,
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:43123/health",
      expect.any(Object),
    );
    expect(append).not.toHaveBeenCalled();
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
