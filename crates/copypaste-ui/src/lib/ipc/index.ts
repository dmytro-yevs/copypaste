// ---------------------------------------------------------------------------
// lib/ipc/index.ts — barrel re-export.
//
// Every import of `../lib/ipc` or `./ipc` in the codebase resolves here
// (via the lib/ipc.ts barrel which re-exports this entire module). Nothing
// in the consumer files needs to change — all public names are preserved.
// ---------------------------------------------------------------------------

export * from "./types";
export * from "./transport";
export * from "./api";
export * from "./tauriCommands";
export * from "./helpers";
