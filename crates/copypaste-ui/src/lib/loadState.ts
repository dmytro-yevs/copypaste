/**
 * loadState.ts — canonical LoadState discriminated union for all views.
 *
 * Three views (HistoryView, SettingsView, DevicesView) independently defined
 * a near-identical LoadState type. This module is the single source of truth.
 * The union is a superset: consumers that only use a subset of states still
 * type-check cleanly because extra union members are harmless widening.
 *
 * Variants:
 *   loading   — in-flight IPC request; spinner shown
 *   ready     — data loaded successfully
 *   offline   — daemon_offline transport error (daemon not running)
 *   not_ready — daemon up but still initialising its database (ipc_not_ready)
 *   degraded  — daemon up but DB unavailable (degraded mode); recovery affordance shown
 *   error     — some other daemon-side failure
 */
export type LoadState =
  | "loading"
  | "ready"
  | "offline"
  | "not_ready"
  | "degraded"
  | "error";

// ---------------------------------------------------------------------------
// Type guards — narrow LoadState to a specific variant
// ---------------------------------------------------------------------------

export function isLoadingState(s: LoadState): s is "loading" {
  return s === "loading";
}

export function isReadyState(s: LoadState): s is "ready" {
  return s === "ready";
}

export function isOfflineState(s: LoadState): s is "offline" {
  return s === "offline";
}

export function isNotReadyState(s: LoadState): s is "not_ready" {
  return s === "not_ready";
}

export function isDegradedState(s: LoadState): s is "degraded" {
  return s === "degraded";
}

export function isErrorState(s: LoadState): s is "error" {
  return s === "error";
}
