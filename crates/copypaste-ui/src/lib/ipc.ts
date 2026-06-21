// ---------------------------------------------------------------------------
// lib/ipc.ts — public re-export barrel.
//
// This file preserves all existing `import ... from "../lib/ipc"` paths
// throughout the codebase. The implementation has been split into:
//
//   lib/ipc/types.ts         — all shared daemon wire types (zero runtime)
//   lib/ipc/transport.ts     — IpcError, ipcCall, mock gate, protocol version
//   lib/ipc/api.ts           — `api` object + MAX_PAGE (socket-bridge wrappers)
//   lib/ipc/tauriCommands.ts — Tauri-direct invoke() wrappers (bypass socket)
//   lib/ipc/helpers.ts       — formatters, probeStatus, detectStale*, isIpcNotReady
//   lib/ipc/index.ts         — barrel re-exporting all of the above
//
// Import paths are UNCHANGED — every consumer that does
//   import { api, IpcError, HistoryEntry, … } from "../lib/ipc"
// continues to resolve here and gets the same export.
// ---------------------------------------------------------------------------

export * from "./ipc/index";
