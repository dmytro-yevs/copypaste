import "@testing-library/jest-dom/vitest";

// ---------------------------------------------------------------------------
// localStorage polyfill — Node 24 compat
//
// Node ≥ 24 ships an experimental process-level `localStorage` global backed
// by --localstorage-file.  When Vitest runs in jsdom mode that global can
// shadow (or conflict with) jsdom's proper Web Storage implementation, causing
// `localStorage.clear()` / `removeItem()` to throw TypeError because the Node
// object doesn't satisfy the full Storage interface.
//
// We always define `window.localStorage` here as a clean in-memory
// implementation so every test gets a predictable, isolated store regardless
// of Node version or experimental flags.
// ---------------------------------------------------------------------------
const _lsStore: Record<string, string> = {};
Object.defineProperty(window, "localStorage", {
  value: {
    getItem: (key: string): string | null =>
      Object.prototype.hasOwnProperty.call(_lsStore, key)
        ? _lsStore[key]
        : null,
    setItem: (key: string, value: string): void => {
      _lsStore[key] = String(value);
    },
    removeItem: (key: string): void => {
      delete _lsStore[key];
    },
    clear: (): void => {
      Object.keys(_lsStore).forEach((k) => delete _lsStore[k]);
    },
    key: (n: number): string | null => Object.keys(_lsStore)[n] ?? null,
    get length(): number {
      return Object.keys(_lsStore).length;
    },
  } satisfies Storage,
  writable: true,
  configurable: true,
});

// ---------------------------------------------------------------------------
// Global Tauri jsdom stubs
//
// The Tauri runtime APIs (window.__TAURI_INTERNALS__, @tauri-apps/api/webview,
// @tauri-apps/api/event listen()) all crash in jsdom because they expect a
// native host object injected by the Rust webview. We stub them here so any
// component that calls them in a useEffect doesn't blow up test files that
// are not specifically testing those integration points.
// ---------------------------------------------------------------------------

// Minimal __TAURI_INTERNALS__ stub — enough to satisfy:
//   getCurrentWindow()  → accesses .metadata.currentWindow
//   getCurrentWebview() → uses getCurrentWindow() + .metadata.currentWebview
//   transformCallback() → used by Channel and listen()
//   invoke()            → wrapped by @tauri-apps/api/core (tests mock this separately)
const tauriInternals = {
  metadata: {
    currentWindow: { label: "main" },
    currentWebview: { label: "main", windowLabel: "main" },
    windows: [{ label: "main", scaleFactor: 1 }],
    webviews: [{ label: "main", windowLabel: "main" }],
  },
  transformCallback: (_callback: unknown, _once?: boolean): number => 0,
  invoke: (_cmd: string, _args?: unknown, _options?: unknown): Promise<unknown> =>
    Promise.resolve(undefined),
  convertFileSrc: (src: string) => src,
};

// Assign once; individual tests can override window.__TAURI_INTERNALS__ locally.
if (!("__TAURI_INTERNALS__" in window)) {
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    value: tauriInternals,
    writable: true,
    configurable: true,
  });
}

// Stub __TAURI_EVENT_PLUGIN_INTERNALS__ used by event.js _unlisten() when
// the cleanup function returned by listen() is called on component unmount.
if (!("__TAURI_EVENT_PLUGIN_INTERNALS__" in window)) {
  Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
    value: { unregisterListener: () => {} },
    writable: true,
    configurable: true,
  });
}
