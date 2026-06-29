// settingsSliders.ts
// Step arrays, labels, and defaults for the stepped sliders in SettingsView.
// Extracted from SettingsView.tsx (CopyPaste-g06m.14 split) — values are
// identical; this is a cut/paste, not a rewrite.

/** Return the step value closest to `raw` (by minimum absolute distance). */
export function snapToNearest<T extends number>(steps: readonly T[], raw: number): T {
  let best = 0;
  let bestDist = Math.abs(raw - (steps[0] as number));
  for (let i = 1; i < steps.length; i++) {
    const d = Math.abs(raw - (steps[i] as number));
    if (d < bestDist) { bestDist = d; best = i; }
  }
  return steps[best];
}

// CopyPaste-sqw0: DEFAULT_POPUP_SHORTCUT is the *fallback* initial-render value
// used only while the IPC call to `get_default_popup_shortcut` is in-flight.
// The Rust constant `DEFAULT_POPUP_SHORTCUT` in `src-tauri/src/lib.rs` is the
// authoritative source; the UI fetches it at load time via
// `getDefaultPopupShortcut()` and stores it in `defaultShortcut` state so the
// "reset to default" button always reflects the Rust value, not this literal.
// If you change the value here, change it in Rust too — and the Rust test
// `default_popup_shortcut_value_matches_ts_expectation` will catch any drift.
export const DEFAULT_POPUP_SHORTCUT = "CmdOrCtrl+Shift+V";

// NOTE: step values are BINARY (MiB/GiB, ×1024² / ×1024³) to match the core
// defaults (DEFAULT_MAX_* below) which are also binary. Using decimal here
// would make e.g. the 10 MiB default snap to a 10 MB (10_000_000) step and
// silently persist a ~5% smaller cap — label drift. Labels use "MB"/"GB"
// (the conventional macOS user-facing unit per Apple HIG — same binary values,
// SI suffix). Android settings display "MiB/GiB" (IEC) for the same binary
// values; the suffix convention differs per platform but the underlying bytes
// are identical. bdac.62: intentional platform convention, not a bug.
export const TEXT_SIZE_STEPS_BYTES = [1,2,5,10,15,25,50,100].map((n) => n * 1024 * 1024) as unknown as readonly number[];
export const TEXT_SIZE_LABELS = ["1 MB","2 MB","5 MB","10 MB","15 MB","25 MB","50 MB","100 MB (max)"] as const;

export const IMAGE_SIZE_STEPS_BYTES = [5,10,25,64,128,256,512].map((n) => n * 1024 * 1024) as unknown as readonly number[];
export const IMAGE_SIZE_LABELS = ["5 MB","10 MB","25 MB","64 MB","128 MB","256 MB","512 MB (max)"] as const;

// File-size cap: max is the library hard cap MAX_FILE_BYTES (100 MiB) — the
// single storable ceiling (mirrors crate::file::MAX_FILE_BYTES). Larger values
// are clamped back down by the daemon, so advertising "2 GB" was dishonest.
// The 8 MB step marks the P2P/relay sync ceiling (SYNC_MAX_BLOB_BYTES): files
// above it are kept locally but skipped for sync (see helper text below).
export const FILE_SIZE_STEPS_BYTES = [8,16,25,50,100].map((n) => n * 1024 * 1024) as unknown as readonly number[];
export const FILE_SIZE_LABELS = ["8 MB","16 MB","25 MB","50 MB","100 MB (max)"] as const;

export const QUOTA_STEPS_BYTES = [1,2,5,10,25,50].map((n) => n * 1024 * 1024 * 1024) as unknown as readonly number[];
export const QUOTA_LABELS = ["1 GB","2 GB","5 GB","10 GB","25 GB","50 GB (max)"] as const;

// 3gsk: add 0 (Off) as first step — Android already has this step so users can
// disable auto-wipe on both platforms. 0 means "never auto-wipe sensitive items".
export const SENSITIVE_TTL_STEPS = [0, 10, 30, 60, 5 * 60, 15 * 60, 60 * 60] as const;
export const SENSITIVE_TTL_LABELS = ["Off","10 s","30 s","1 min","5 min","15 min","1 hour"] as const;

// History display limit slider — controls how many items the UI renders on screen.
// This is a UI-only preference persisted in localStorage. It does NOT cap daemon
// storage; the daemon prunes by byte quota (storage_quota_bytes), not item count.
// Label: "History display limit" so users understand it's a view filter, not retention.
export const MAX_ITEMS_STEPS = [100, 250, 500, 1000, 2500, 5000, 10000, 100000] as const;
export const MAX_ITEMS_LABELS = ["100","250","500","1 000","2 500","5 000","10 000","Unlimited"] as const;
export const DEFAULT_MAX_ITEMS = 1000; // default UI display window

// ---------------------------------------------------------------------------
// Storage / Limits defaults — MUST mirror copypaste-core
// (crates/copypaste-core/src/config/defaults.rs). Stepped-slider state is now
// stored as raw bytes (or item count / seconds) snapped to the nearest step
// array entry so an existing config always loads cleanly.
//
// Core binary defaults (MiB/GiB):
//   text 10 MiB, image 64 MiB, file 100 MiB, quota 10 GiB
// Step arrays defined above (moved from the deleted StepSlider.tsx in v0.5.3) cover or exceed each of these.
export const DEFAULT_MAX_TEXT_BYTES = 10 * 1024 * 1024;          // 10 MiB
export const DEFAULT_MAX_IMAGE_BYTES = 64 * 1024 * 1024;          // 64 MiB
export const DEFAULT_MAX_FILE_BYTES = 100 * 1024 * 1024;          // 100 MiB (= crate::file::MAX_FILE_BYTES, the storable hard cap)
export const DEFAULT_STORAGE_QUOTA_BYTES = 10 * 1024 * 1024 * 1024; // 10 GiB
export const DEFAULT_SENSITIVE_TTL_SECS = 30;
