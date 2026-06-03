import { useCallback, useEffect, useRef, useState } from "react";
import ReactDOM from "react-dom";
import { emit, listen } from "@tauri-apps/api/event";
import { ViewShell } from "../components/ViewShell";
import {
  api,
  IpcError,
  appVersion,
  getPopupShortcut,
  setPopupShortcut,
  restartDaemon,
  detectStaleDaemonFromStatus,
  type AppSettings,
  type SyncStatus,
  type DaemonStatus,
} from "../lib/ipc";
import { RestartDaemonButton } from "../components/RestartDaemonButton";
import { useUI } from "../store";
// Step arrays (moved from StepSlider.tsx — StepSlider component deleted in v0.5.3,
// all sliders now use the unified SliderRow component).

/** Return the step value closest to `raw` (by minimum absolute distance). */
function snapToNearest<T extends number>(steps: readonly T[], raw: number): T {
  let best = 0;
  let bestDist = Math.abs(raw - (steps[0] as number));
  for (let i = 1; i < steps.length; i++) {
    const d = Math.abs(raw - (steps[i] as number));
    if (d < bestDist) { bestDist = d; best = i; }
  }
  return steps[best];
}

// Default popup shortcut. Mirrors DEFAULT_POPUP_SHORTCUT in
// src-tauri/src/lib.rs (the Rust default is not exposed over IPC, so it is
// duplicated here for the "reset to default" button). Keep the two in sync.
const DEFAULT_POPUP_SHORTCUT = "CmdOrCtrl+Shift+V";

// NOTE: step values are BINARY (MiB/GiB, ×1024² / ×1024³) to match the core
// defaults (DEFAULT_MAX_* below) which are also binary. Using decimal here
// would make e.g. the 10 MiB default snap to a 10 MB (10_000_000) step and
// silently persist a ~5% smaller cap — label drift. Labels keep "MB"/"GB"
// (MB-as-MiB is the common app convention) while the values are binary.
const TEXT_SIZE_STEPS_BYTES = [1,2,5,10,15,25,50,100].map((n) => n * 1024 * 1024) as unknown as readonly number[];
const TEXT_SIZE_LABELS = ["1 MB","2 MB","5 MB","10 MB","15 MB","25 MB","50 MB","100 MB (max)"] as const;

const IMAGE_SIZE_STEPS_BYTES = [5,10,25,64,128,256,512].map((n) => n * 1024 * 1024) as unknown as readonly number[];
const IMAGE_SIZE_LABELS = ["5 MB","10 MB","25 MB","64 MB","128 MB","256 MB","512 MB (max)"] as const;

// File-size cap: max is the library hard cap MAX_FILE_BYTES (100 MiB) — the
// single storable ceiling (mirrors crate::file::MAX_FILE_BYTES). Larger values
// are clamped back down by the daemon, so advertising "2 GB" was dishonest.
// The 8 MB step marks the P2P/relay sync ceiling (SYNC_MAX_BLOB_BYTES): files
// above it are kept locally but skipped for sync (see helper text below).
const FILE_SIZE_STEPS_BYTES = [8,16,25,50,100].map((n) => n * 1024 * 1024) as unknown as readonly number[];
const FILE_SIZE_LABELS = ["8 MB","16 MB","25 MB","50 MB","100 MB (max)"] as const;

const QUOTA_STEPS_BYTES = [1,2,5,10,25,50].map((n) => n * 1024 * 1024 * 1024) as unknown as readonly number[];
const QUOTA_LABELS = ["1 GB","2 GB","5 GB","10 GB","25 GB","50 GB (max)"] as const;

const SENSITIVE_TTL_STEPS = [10, 30, 60, 5 * 60, 15 * 60, 60 * 60] as const;
const SENSITIVE_TTL_LABELS = ["10 s","30 s","1 min","5 min","15 min","1 hour"] as const;

// ---------------------------------------------------------------------------
// Toggle — iOS-style switch using ide tokens
// ---------------------------------------------------------------------------

function Toggle({
  checked,
  onChange,
  disabled,
  "aria-label": ariaLabel,
}: {
  checked: boolean;
  onChange: (val: boolean) => void;
  disabled?: boolean;
  "aria-label"?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={[
        "relative inline-flex h-[18px] w-[34px] shrink-0 cursor-pointer items-center rounded-full",
        "border focus:outline-none focus:ring-2 focus:ring-ide-accent/50 focus:ring-offset-1 focus:ring-offset-ide-bg",
        "disabled:cursor-not-allowed disabled:opacity-40",
        checked
          ? "border-ide-accent bg-ide-accent shadow-[0_0_6px_rgba(53,146,255,0.30)]"
          : "border-ide-border bg-ide-elevated",
      ].join(" ")}
    >
      <span
        className={[
          "inline-block h-[12px] w-[12px] rounded-full bg-white shadow-ide-xs",
          "transition-transform duration-[120ms] ease",
          checked ? "translate-x-[18px]" : "translate-x-[2px]",
        ].join(" ")}
      />
    </button>
  );
}

// ---------------------------------------------------------------------------
// Shared layout primitives
// ---------------------------------------------------------------------------

function SubsectionHeader({ label, hint }: { label: string; hint?: string }) {
  return (
    <div className="mb-1.5 mt-7 first:mt-0">
      <div className="text-[10px] font-semibold uppercase tracking-wider text-ide-accent/80">{label}</div>
      {hint && <div className="mt-0.5 text-[11px] text-ide-faint">{hint}</div>}
    </div>
  );
}

function SettingsRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex min-h-[36px] items-center justify-between border-b border-ide-divider/70 px-3 py-2 last:border-b-0">
      {/* W4-3: fixed min-width on label column prevents wrapping on narrow labels */}
      <span className="min-w-[160px] shrink-0 text-[13px] text-ide-text">{label}</span>
      <div className="flex items-center gap-2">{children}</div>
    </div>
  );
}

function Panel({ children }: { children: React.ReactNode }) {
  // HW-M3: overflow-hidden was clipping the absolutely-positioned InfoPopover (z-50).
  // The outer div keeps the border/shadow/rounding; an inner div clips the row
  // bottom-borders to the panel's rounded corners without clipping the popover,
  // which floats above the outer div via z-50.
  return (
    <div className="rounded-ide-lg border border-ide-border bg-ide-elevated shadow-ide-sm">
      <div className="overflow-hidden rounded-ide-lg">
        {children}
      </div>
    </div>
  );
}

function StatusRow({ label, ok }: { label: string; ok: boolean }) {
  return (
    <div className="flex items-center gap-2 text-[13px] text-ide-dim">
      <span className="w-[140px] shrink-0">{label}</span>
      <span className={ok ? "text-ide-success" : "text-ide-faint"}>
        {ok ? "✓" : "—"}
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// W4-2: Slider row — consistent grid: [slider (flex)] [fixed-width value]
// Extended in v0.5.3 with optional onRelease to save only on mouse-up/touch-end
// (prevents spamming IPC on every drag tick in storage sliders).
// ---------------------------------------------------------------------------

function SliderRow({
  min,
  max,
  step,
  value,
  onChange,
  onRelease,
  formatValue,
  disabled,
}: {
  min: number;
  max: number;
  step: number;
  value: number;
  onChange: (v: number) => void;
  /** Called on mouse-up / touch-end / key-up — saves to daemon without spamming. */
  onRelease?: (v: number) => void;
  /** Format the numeric value for the right-hand value label. */
  formatValue: (v: number) => string;
  disabled?: boolean;
}) {
  // HW-M4: compute fill % for the accent-colored track. Since appearance:none
  // disables native accent-color, we drive the gradient via a CSS custom prop.
  const pct = max === min ? 0 : ((value - min) / (max - min)) * 100;
  return (
    <div className="flex items-center gap-2">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        onMouseUp={(e) => onRelease?.(Number((e.target as HTMLInputElement).value))}
        onTouchEnd={(e) => onRelease?.(Number((e.currentTarget as HTMLInputElement).value))}
        onKeyUp={(e) => onRelease?.(Number((e.target as HTMLInputElement).value))}
        className="w-28 disabled:opacity-40 disabled:cursor-not-allowed"
        style={{ ["--_fill" as string]: `${pct}%` }}
      />
      <span className="w-[52px] text-right text-[13px] text-ide-text">
        {formatValue(value)}
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// InfoPopover — collapsible help text behind a ⓘ icon (M8)
// HW-M3 fix: popover content is rendered via ReactDOM.createPortal to
// document.body so it can never be clipped by an ancestor overflow-hidden div.
// Position is computed from the trigger button's getBoundingClientRect.
// Click outside to close.
// ---------------------------------------------------------------------------

function InfoPopover({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number }>({ top: 0, left: 0 });
  const btnRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  // Recompute position from the trigger button each time it opens.
  const handleToggle = useCallback(() => {
    if (!open && btnRef.current) {
      const rect = btnRef.current.getBoundingClientRect();
      // Place popover to the right of the icon, vertically centered on it.
      setPos({
        top: rect.top + rect.height / 2,
        left: rect.right + 6,
      });
    }
    setOpen((v) => !v);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node;
      const outsideBtn = btnRef.current && !btnRef.current.contains(target);
      const outsidePopover = popoverRef.current && !popoverRef.current.contains(target);
      if (outsideBtn && outsidePopover) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const popoverEl = open
    ? ReactDOM.createPortal(
        <div
          ref={popoverRef}
          className="z-[9999] w-56 rounded-ide border border-ide-border bg-ide-elevated p-2 text-[11px] text-ide-dim shadow-ide-sm"
          style={{
            position: "fixed",
            top: pos.top,
            left: pos.left,
            minWidth: "14rem",
            transform: "translateY(-50%)",
          }}
        >
          {text}
        </div>,
        document.body
      )
    : null;

  return (
    <div className="inline-flex items-center">
      <button
        ref={btnRef}
        type="button"
        aria-label="More info"
        aria-expanded={open}
        onClick={handleToggle}
        className="flex h-4 w-4 items-center justify-center rounded-full text-ide-faint hover:text-ide-dim transition-colors"
      >
        <svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor" aria-hidden="true">
          <path d="M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1Zm0 3a.9.9 0 1 1 0 1.8A.9.9 0 0 1 8 4Zm-.75 2.75h1.5v4.5h-1.5v-4.5Z" />
        </svg>
      </button>
      {popoverEl}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Tab bar
// ---------------------------------------------------------------------------

type TabId = "general" | "display" | "sync" | "shortcuts" | "storage" | "advanced";

const TABS: { id: TabId; label: string }[] = [
  { id: "general",   label: "General"   },
  { id: "display",   label: "Display"   },
  { id: "sync",      label: "Sync"      },
  { id: "shortcuts", label: "Shortcuts" },
  { id: "storage",   label: "Storage"   },
  { id: "advanced",  label: "Advanced"  },
];

function TabBar({
  active,
  onChange,
}: {
  active: TabId;
  onChange: (id: TabId) => void;
}) {
  return (
    <div className="mb-4 flex gap-0.5 border-b border-ide-border pb-0">
      {TABS.map((t) => (
        <button
          key={t.id}
          type="button"
          onClick={() => onChange(t.id)}
          className={[
            "px-3 py-2 text-[13px] transition-colors",
            "border-b-2 -mb-px",
            active === t.id
              ? "border-ide-accent text-ide-text font-medium"
              : "border-transparent text-ide-dim hover:text-ide-text",
          ].join(" ")}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatLastSync(ms: number | null): string {
  if (ms === null) return "Never";
  const diff = Date.now() - ms;
  if (diff < 60_000) return "Just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return new Date(ms).toLocaleString();
}

// ---------------------------------------------------------------------------
// Storage / Limits defaults — MUST mirror copypaste-core
// (crates/copypaste-core/src/config/defaults.rs). Stepped-slider state is now
// stored as raw bytes (or item count / seconds) snapped to the nearest step
// array entry so an existing config always loads cleanly.
//
// Core binary defaults (MiB/GiB):
//   text 10 MiB, image 64 MiB, file 100 MiB, quota 10 GiB
// Step arrays defined above (moved from the deleted StepSlider.tsx in v0.5.3) cover or exceed each of these.
const DEFAULT_MAX_TEXT_BYTES = 10 * 1024 * 1024;          // 10 MiB
const DEFAULT_MAX_IMAGE_BYTES = 64 * 1024 * 1024;          // 64 MiB
const DEFAULT_MAX_FILE_BYTES = 100 * 1024 * 1024;          // 100 MiB (= crate::file::MAX_FILE_BYTES, the storable hard cap)
const DEFAULT_STORAGE_QUOTA_BYTES = 10 * 1024 * 1024 * 1024; // 10 GiB
const DEFAULT_IMAGE_QUALITY = 100;
const DEFAULT_SENSITIVE_TTL_SECS = 30;

// ---------------------------------------------------------------------------
// ShortcutCapture — focus to record a new key combo
// ---------------------------------------------------------------------------

/** Convert a KeyboardEvent into a Tauri accelerator string like "CmdOrCtrl+Shift+V". */
function eventToAccelerator(e: React.KeyboardEvent<HTMLInputElement>): string | null {
  // Ignore bare modifier keydowns (nothing to bind yet).
  if (["Meta", "Control", "Alt", "Shift"].includes(e.key)) return null;

  const parts: string[] = [];
  // On macOS Cmd maps to Meta; on other platforms Ctrl maps to CmdOrCtrl.
  // Tauri accepts "CmdOrCtrl" as a cross-platform alias.
  if (e.metaKey || e.ctrlKey) parts.push("CmdOrCtrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");

  // Always derive from the PHYSICAL key (e.code), not e.key, so the shortcut
  // is keyboard-layout-independent (e.g. Cyrillic layouts still record "Q").
  let key: string;
  if (e.code.startsWith("Key")) {
    key = e.code.slice(3); // "KeyQ" → "Q"
  } else if (e.code.startsWith("Digit")) {
    key = e.code.slice(5); // "Digit1" → "1"
  } else {
    key = e.code || e.key;
  }

  if (key.length === 1) {
    key = key.toUpperCase();
  } else {
    const keyMap: Record<string, string> = {
      ArrowUp: "Up",
      ArrowDown: "Down",
      ArrowLeft: "Left",
      ArrowRight: "Right",
      " ": "Space",
      Space: "Space",
      Escape: "Escape",
      Enter: "Return",
      Return: "Return",
      Backspace: "Backspace",
      Delete: "Delete",
      Tab: "Tab",
      Home: "Home",
      End: "End",
      PageUp: "PageUp",
      PageDown: "PageDown",
      F1: "F1",
      F2: "F2",
      F3: "F3",
      F4: "F4",
      F5: "F5",
      F6: "F6",
      F7: "F7",
      F8: "F8",
      F9: "F9",
      F10: "F10",
      F11: "F11",
      F12: "F12",
    };
    key = keyMap[key] ?? key;
  }
  // Require at least one modifier for a meaningful global shortcut.
  if (parts.length === 0) return null;

  parts.push(key);
  return parts.join("+");
}

function ShortcutCapture({
  value,
  onChange,
}: {
  value: string;
  onChange: (accel: string) => void;
}) {
  const [capturing, setCapturing] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setCapturing(false);
        inputRef.current?.blur();
        return;
      }
      const accel = eventToAccelerator(e);
      if (accel !== null) {
        onChange(accel);
        setCapturing(false);
        inputRef.current?.blur();
      }
    },
    [onChange]
  );

  return (
    <input
      ref={inputRef}
      readOnly
      value={capturing ? "Press a shortcut…" : value}
      onFocus={() => setCapturing(true)}
      onBlur={() => setCapturing(false)}
      onKeyDown={handleKeyDown}
      className={[
        "w-48 cursor-pointer rounded-ide border px-2.5 py-1.5 text-[13px] text-ide-text",
        "outline-none select-none bg-ide-bg",
        capturing
          ? "border-ide-accent ring-1 ring-ide-accent"
          : "border-ide-border hover:border-ide-accent",
      ].join(" ")}
      title="Click and press a key combination"
    />
  );
}

// ---------------------------------------------------------------------------
// Main view
// ---------------------------------------------------------------------------

// `degraded` = daemon up but its DB is unavailable (reported only by `status`).
// Distinct from `offline` so the banner is accurate and the inputs that need a
// working DB stay disabled.
type LoadState = "loading" | "ready" | "offline" | "degraded";

export function SettingsView() {
  // Display prefs (localStorage-persisted, no daemon needed).
  // Each field has its own selector returning a STABLE reference — a single
  // selector returning `{ prefs, setPrefs }` creates an unstable snapshot under
  // Zustand v5 + useSyncExternalStore, which blanked the window on open.
  const prefs = useUI((s) => s.prefs);
  const setPrefs = useUI((s) => s.setPrefs);

  const [activeTab, setActiveTab] = useState<TabId>("general");

  // General
  const [privateMode, setPrivateMode] = useState(false);

  // Sync / cloud config
  const [config, setConfig] = useState<AppSettings>({
    p2p_enabled: true,
    supabase_url: null,
    supabase_anon_key: null,
  });
  const [supabaseUrl, setSupabaseUrl] = useState("");
  const [supabaseKey, setSupabaseKey] = useState("");
  const [relayUrl, setRelayUrl] = useState("");
  const [savedMsg, setSavedMsg] = useState(false);
  const [testMsg, setTestMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const [testing, setTesting] = useState(false);

  // Cloud sync passphrase
  const [passphrase, setPassphrase] = useState("");
  const [passphraseSavedMsg, setPassphraseSavedMsg] = useState<string | null>(null);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);

  // Shortcuts
  const [currentShortcut, setCurrentShortcut] = useState(DEFAULT_POPUP_SHORTCUT);
  const [pendingShortcut, setPendingShortcut] = useState(DEFAULT_POPUP_SHORTCUT);
  const [shortcutMsg, setShortcutMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const shortcutTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Storage / Limits — stepped slider state stored in raw units (bytes, items, seconds).
  // Each value is snapped to the nearest step array entry on load and on change.
  const [maxTextBytes, setMaxTextBytes] = useState(
    snapToNearest(TEXT_SIZE_STEPS_BYTES as unknown as readonly number[], DEFAULT_MAX_TEXT_BYTES)
  );
  const [maxImageBytes, setMaxImageBytes] = useState(
    snapToNearest(IMAGE_SIZE_STEPS_BYTES as unknown as readonly number[], DEFAULT_MAX_IMAGE_BYTES)
  );
  const [maxFileBytes, setMaxFileBytes] = useState(
    snapToNearest(FILE_SIZE_STEPS_BYTES as unknown as readonly number[], DEFAULT_MAX_FILE_BYTES)
  );
  const [quotaBytes, setQuotaBytes] = useState(
    snapToNearest(QUOTA_STEPS_BYTES as unknown as readonly number[], DEFAULT_STORAGE_QUOTA_BYTES)
  );
  const [sensitiveTtlSecs, setSensitiveTtlSecs] = useState(
    snapToNearest(SENSITIVE_TTL_STEPS as unknown as readonly number[], DEFAULT_SENSITIVE_TTL_SECS)
  );
  const [imageQuality, setImageQuality] = useState(DEFAULT_IMAGE_QUALITY);
  // Per-field save feedback: key = field name, value = error or "Saved" / null.
  const [limitsMsg, setLimitsMsg] = useState<Record<string, string | null>>({});
  const limitsMsgTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({});

  // Sync parity — p2p toggle + wifi-only
  const [syncOnWifiOnly, setSyncOnWifiOnly] = useState(false);

  // Privacy & capture — daemon AppConfig fields (config.toml).
  const [collectPublicIp, setCollectPublicIp] = useState(true);
  const [pasteAsPlainText, setPasteAsPlainText] = useState(false);
  const [excludedApps, setExcludedApps] = useState<string[]>([]);
  // Text buffer for the "add excluded app" input.
  const [newExcludedApp, setNewExcludedApp] = useState("");

  // Sync-path restart guard: true while restart_daemon is in flight after a
  // sync-path toggle (P2P/relay/Supabase). Disables the control so rapid
  // double-toggles can't queue two restarts.
  const [syncRestarting, setSyncRestarting] = useState(false);
  const syncRestartTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Data
  const [deleteMsg, setDeleteMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState(false);

  // Global state
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [degradedReason, setDegradedReason] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [staleDaemon, setStaleDaemon] = useState<string | null>(null);
  const [daemonVersion, setDaemonVersion] = useState<string | null>(null);

  // Save-config error
  const [saveError, setSaveError] = useState<string | null>(null);
  const saveErrTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Private-mode error
  const [privateModeError, setPrivateModeError] = useState<string | null>(null);
  const pmErrTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const savedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const passphraseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Clear every handler-scheduled feedback timer on unmount so a late tick
  // never calls setState on an unmounted component (UI memory leak). These
  // timers are started inside event handlers (Save / shortcut / delete / etc.),
  // so an effect cleanup is the only place that runs on unmount.
  useEffect(() => {
    return () => {
      if (shortcutTimerRef.current !== null) clearTimeout(shortcutTimerRef.current);
      if (saveErrTimer.current !== null) clearTimeout(saveErrTimer.current);
      if (pmErrTimer.current !== null) clearTimeout(pmErrTimer.current);
      if (savedTimerRef.current !== null) clearTimeout(savedTimerRef.current);
      if (deleteTimerRef.current !== null) clearTimeout(deleteTimerRef.current);
      if (passphraseTimerRef.current !== null) clearTimeout(passphraseTimerRef.current);
      for (const t of Object.values(limitsMsgTimers.current)) clearTimeout(t);
    };
  }, []);

  // -------------------------------------------------------------------------
  // Load
  // -------------------------------------------------------------------------

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setLoadState("loading");
      // Popup shortcut is Tauri-direct — works even when daemon is offline.
      getPopupShortcut()
        .then((s) => {
          if (cancelled) return;
          setCurrentShortcut(s);
          setPendingShortcut(s);
        })
        .catch(() => {
          // Keep default if Tauri command fails (shouldn't happen in normal operation).
        });

      try {
        // api.status() is fetched once and reused for: degraded probe, build_version
        // display, and stale-daemon detection — avoids three separate round-trips.
        const [pmResult, cfg, syncSt, daemonSt, myAppVer] = await Promise.all([
          api.getPrivateMode().catch(() => null),
          api.getConfig().catch(() => null),
          api.getSyncStatus().catch(() => null),
          api.status().catch(() => null) as Promise<DaemonStatus | null>,
          appVersion().catch(() => null),
        ]);
        if (cancelled) return;

        const probe = daemonSt
          ? (daemonSt.degraded === true || daemonSt.ready === false
              ? { kind: "degraded" as const, reason: daemonSt.degraded_reason ?? null }
              : { kind: "ok" as const })
          : { kind: "offline" as const };

        setDegradedReason(probe.kind === "degraded" ? (probe.reason ?? "") : null);
        setDaemonVersion(daemonSt?.build_version ?? null);
        if (myAppVer !== null) {
          setStaleDaemon(detectStaleDaemonFromStatus(daemonSt, myAppVer));
        }

        if (pmResult === null && cfg === null) {
          setLoadState(probe.kind === "degraded" ? "degraded" : "offline");
          setSyncStatus(syncSt);
          return;
        }

        setPrivateMode(pmResult?.private_mode ?? false);

        // Hydrate all AppConfig-backed fields from get_config response.
        const rawCfg = cfg ?? ({} as Partial<AppSettings>);
        setConfig({
          p2p_enabled: rawCfg.p2p_enabled ?? true,
          supabase_url: rawCfg.supabase_url ?? null,
          supabase_anon_key: rawCfg.supabase_anon_key ?? null,
          relay_url: rawCfg.relay_url ?? null,
        });

        // Prefill Supabase URL — prefer stored config, fall back to sync_status.
        setSupabaseUrl(rawCfg.supabase_url ?? syncSt?.supabase_url ?? "");
        setSupabaseKey(rawCfg.supabase_anon_key ?? "");
        setRelayUrl(rawCfg.relay_url ?? "");
        setSyncStatus(syncSt);

        // Storage / Limits — snap raw bytes to nearest step array entry so an
        // existing config with an arbitrary value always loads cleanly.
        setMaxTextBytes(snapToNearest(
          TEXT_SIZE_STEPS_BYTES as unknown as readonly number[],
          rawCfg.max_text_size_bytes ?? DEFAULT_MAX_TEXT_BYTES
        ));
        setMaxImageBytes(snapToNearest(
          IMAGE_SIZE_STEPS_BYTES as unknown as readonly number[],
          rawCfg.max_image_size_bytes ?? DEFAULT_MAX_IMAGE_BYTES
        ));
        setMaxFileBytes(snapToNearest(
          FILE_SIZE_STEPS_BYTES as unknown as readonly number[],
          rawCfg.max_file_size_bytes ?? DEFAULT_MAX_FILE_BYTES
        ));
        setQuotaBytes(snapToNearest(
          QUOTA_STEPS_BYTES as unknown as readonly number[],
          rawCfg.storage_quota_bytes ?? DEFAULT_STORAGE_QUOTA_BYTES
        ));
        setSensitiveTtlSecs(snapToNearest(
          SENSITIVE_TTL_STEPS as unknown as readonly number[],
          rawCfg.sensitive_ttl_secs ?? DEFAULT_SENSITIVE_TTL_SECS
        ));
        setImageQuality(rawCfg.image_quality ?? DEFAULT_IMAGE_QUALITY);

        // Sync parity
        setSyncOnWifiOnly(rawCfg.sync_on_wifi_only ?? false);

        // Privacy & capture — these AppConfig fields are not in the AppSettings
        // interface (kept in lib/ipc.ts), so read them off the raw response with
        // a narrow typed view rather than `any`.
        const privacyCfg = rawCfg as {
          collect_public_ip?: boolean | null;
          paste_as_plain_text?: boolean | null;
          excluded_app_bundle_ids?: string[] | null;
        };
        setCollectPublicIp(privacyCfg.collect_public_ip ?? true);
        setPasteAsPlainText(privacyCfg.paste_as_plain_text ?? false);
        setExcludedApps(privacyCfg.excluded_app_bundle_ids ?? []);

        // Guard again — a second reloadKey bump that fired while we were
        // awaiting could have set cancelled=true between the check above and
        // the Zustand setPrefs calls below.  Without this, two in-flight loads
        // can interleave and the stale response wins the last write.
        if (cancelled) return;

        // Sound / notify — hydrate from daemon config so UI reflects persisted state.
        if (rawCfg.sound_on_copy != null) {
          setPrefs({ playSoundOnCopy: rawCfg.sound_on_copy });
        }
        if (rawCfg.notify_on_copy != null) {
          setPrefs({ notifyOnCopy: rawCfg.notify_on_copy });
        }

        setLoadState("ready");
      } catch (err) {
        if (cancelled) return;
        void err;
        setLoadState("offline");
      }
    }

    void load();
    return () => {
      cancelled = true;
      if (syncRestartTimerRef.current !== null) clearTimeout(syncRestartTimerRef.current);
    };
  }, [reloadKey]);

  // Re-sync the daemon-backed Private mode toggle whenever the window regains
  // focus or becomes visible. The value is loaded once on mount, but if the
  // daemon was slow/degraded then (or the user changed it from the tray menu
  // while Settings was in the background) the toggle would show a stale value
  // and diverge from the tray — which has its own resync poller. Re-fetching on
  // focus/visibility makes Settings reflect daemon truth like the tray does.
  useEffect(() => {
    let cancelled = false;

    const resyncPrivateMode = () => {
      api
        .getPrivateMode()
        .then((result) => {
          if (!cancelled && result) setPrivateMode(result.private_mode);
        })
        .catch(() => {
          // Best-effort — leave the current toggle value on transient failure.
        });
    };

    const onVisibility = () => {
      if (document.visibilityState === "visible") resyncPrivateMode();
    };

    window.addEventListener("focus", resyncPrivateMode);
    document.addEventListener("visibilitychange", onVisibility);

    // M4: When the toggle originates from the tray menu, the backend re-emits
    // `private-mode-changed` so this window converges without waiting for a
    // focus/visibility re-fetch. Keep the local React state in sync.
    const unlistenPromise = listen<boolean>("private-mode-changed", (event) => {
      if (!cancelled && typeof event.payload === "boolean") {
        setPrivateMode(event.payload);
      }
    });

    return () => {
      cancelled = true;
      window.removeEventListener("focus", resyncPrivateMode);
      document.removeEventListener("visibilitychange", onVisibility);
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const offline = loadState !== "ready";
  const degraded = loadState === "degraded";

  // -------------------------------------------------------------------------
  // Helpers — per-field limits save with feedback
  // -------------------------------------------------------------------------

  function showLimitsMsg(field: string, msg: string | null, durationMs: number) {
    if (limitsMsgTimers.current[field] !== undefined) {
      clearTimeout(limitsMsgTimers.current[field]);
    }
    setLimitsMsg((prev) => ({ ...prev, [field]: msg }));
    if (msg !== null) {
      limitsMsgTimers.current[field] = setTimeout(
        () => setLimitsMsg((prev) => ({ ...prev, [field]: null })),
        durationMs
      );
    }
  }

  // Privacy & capture fields that are not (yet) in the AppSettings interface in
  // lib/ipc.ts. set_config accepts them; we attach them via this typed shape so
  // every patch round-trips the current privacy state without using `any`.
  type PrivacyPatch = {
    collect_public_ip?: boolean | null;
    paste_as_plain_text?: boolean | null;
    excluded_app_bundle_ids?: string[] | null;
  };

  // Build the full AppSettings patch for set_config, merging current config
  // with any updated limits fields. Slider values are already raw bytes/counts/secs.
  function buildConfigPatch(overrides: Partial<AppSettings> & PrivacyPatch): AppSettings & PrivacyPatch {
    return {
      p2p_enabled: config.p2p_enabled,
      supabase_url: supabaseUrl.trim() || null,
      supabase_anon_key: supabaseKey.trim() || null,
      relay_url: relayUrl.trim() || null,
      max_text_size_bytes: maxTextBytes,
      max_image_size_bytes: maxImageBytes,
      max_file_size_bytes: maxFileBytes,
      storage_quota_bytes: quotaBytes,
      sensitive_ttl_secs: sensitiveTtlSecs,
      image_quality: imageQuality,
      sync_on_wifi_only: syncOnWifiOnly,
      sound_on_copy: prefs.playSoundOnCopy,
      notify_on_copy: prefs.notifyOnCopy,
      collect_public_ip: collectPublicIp,
      paste_as_plain_text: pasteAsPlainText,
      excluded_app_bundle_ids: excludedApps,
      ...overrides,
    };
  }

  // Add a bundle ID to the excluded-apps list and persist immediately. Computes
  // the next list explicitly (React state updates are async) so the set_config
  // patch carries the new value, not the stale one. Reverts on failure.
  async function addExcludedApp() {
    const id = newExcludedApp.trim();
    if (id === "" || excludedApps.includes(id)) {
      setNewExcludedApp("");
      return;
    }
    const next = [...excludedApps, id];
    const prev = excludedApps;
    setExcludedApps(next);
    setNewExcludedApp("");
    if (loadState !== "ready") return;
    try {
      await api.setConfig(
        buildConfigPatch({ excluded_app_bundle_ids: next }) as unknown as Parameters<typeof api.setConfig>[0],
      );
    } catch {
      setExcludedApps(prev);
    }
  }

  // Remove a bundle ID from the excluded-apps list and persist. Reverts on failure.
  async function removeExcludedApp(bundleId: string) {
    const next = excludedApps.filter((b) => b !== bundleId);
    const prev = excludedApps;
    setExcludedApps(next);
    if (loadState !== "ready") return;
    try {
      await api.setConfig(
        buildConfigPatch({ excluded_app_bundle_ids: next }) as unknown as Parameters<typeof api.setConfig>[0],
      );
    } catch {
      setExcludedApps(prev);
    }
  }

  // P1 fix: saveLimitsField now accepts an optional per-field revert callback so
  // it can undo only the specific field that failed, instead of triggering a full
  // reload (setReloadKey) which resets ALL sliders from scratch.
  async function saveLimitsField(
    field: string,
    patch: Partial<AppSettings>,
    onRevert?: () => void,
  ) {
    try {
      await api.setConfig(buildConfigPatch(patch) as unknown as Parameters<typeof api.setConfig>[0]);
      showLimitsMsg(field, "Saved", 2000);
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Save failed";
      showLimitsMsg(field, msg, 4000);
      // Revert only the specific field that failed, not all sliders.
      onRevert?.();
    }
  }

  // -------------------------------------------------------------------------
  // General — Private mode
  // -------------------------------------------------------------------------

  const handlePrivateMode = useCallback(
    async (val: boolean) => {
      // Optimistic update so the toggle responds immediately.
      setPrivateMode(val);
      setPrivateModeError(null);
      try {
        // Daemon echoes back the confirmed value — use it so the displayed
        // state always matches the actual daemon state, never an assumption.
        const result = await api.setPrivateMode(val);
        setPrivateMode(result.private_mode);
        // M4: Push the daemon-confirmed value to the tray so its CheckMenuItem
        // refreshes immediately instead of showing a stale cached state. Emit
        // the confirmed value, never the optimistic pre-toggle one.
        void emit("private-mode-changed", result.private_mode);
      } catch (err) {
        // Revert on failure and surface the error.
        setPrivateMode(!val);
        const msg = err instanceof IpcError ? err.message : "Failed to update private mode";
        setPrivateModeError(msg);
        if (pmErrTimer.current !== null) clearTimeout(pmErrTimer.current);
        pmErrTimer.current = setTimeout(() => setPrivateModeError(null), 3500);
      }
    },
    [pmErrTimer]
  );

  // -------------------------------------------------------------------------
  // Sync — Save config (URL + anon key + p2p_enabled)
  // -------------------------------------------------------------------------

  const handleSaveConfig = useCallback(async () => {
    // V-9 fix: only send supabase_anon_key when the user has actually typed a
    // new value.  If the field is blank AND the daemon already has a key stored
    // (config.supabase_anon_key !== null), omit the field from the payload so
    // the daemon's merge_config preserves the stored key.  Sending null would
    // silently overwrite it — the field shows a "set ✓" placeholder in the UI
    // precisely because get_config returns the key but the input stays empty
    // when the user hasn't changed it.
    const trimmedKey = supabaseKey.trim();
    const anonKey: string | null =
      trimmedKey !== ""
        ? trimmedKey
        : config.supabase_anon_key; // preserve existing; null only if never set

    const next: AppSettings = {
      p2p_enabled: config.p2p_enabled,
      supabase_url: supabaseUrl.trim() || null,
      supabase_anon_key: anonKey,
      relay_url: relayUrl.trim() || null,
    };
    setSaveError(null);
    try {
      await api.setConfig(next);
      setConfig(next);
      setSavedMsg(true);
      if (savedTimerRef.current !== null) clearTimeout(savedTimerRef.current);
      savedTimerRef.current = setTimeout(() => setSavedMsg(false), 2500);
      // Supabase URL/key are read at daemon startup — restart so the new
      // credentials take effect immediately without requiring a manual relaunch.
      setSyncRestarting(true);
      try {
        await restartDaemon();
      } catch {
        // Non-fatal: config is saved; user can relaunch manually if restart fails.
      } finally {
        setSyncRestarting(false);
      }
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Save failed";
      setSaveError(msg);
      if (saveErrTimer.current !== null) clearTimeout(saveErrTimer.current);
      saveErrTimer.current = setTimeout(() => setSaveError(null), 3500);
    }
  }, [config.p2p_enabled, config.supabase_anon_key, supabaseUrl, supabaseKey, relayUrl, saveErrTimer]);

  const handleTestConnection = useCallback(async () => {
    setTesting(true);
    setTestMsg(null);
    try {
      await handleSaveConfig();
      const result = await api.testCloudConnection();
      setTestMsg({ text: result.message, ok: result.ok });
    } catch (err) {
      const msg =
        err instanceof IpcError
          ? err.message
          : "Connection test unavailable (daemon offline or cloud-sync not built in)";
      setTestMsg({ text: msg, ok: false });
    } finally {
      setTesting(false);
    }
  }, [handleSaveConfig]);

  // -------------------------------------------------------------------------
  // Sync parity — p2p toggle + wifi-only
  // -------------------------------------------------------------------------

  // NOT memoized: buildConfigPatch reads live component state (sliders,
  // supabase fields) via closure. Memoizing on a narrow dep list would freeze
  // a stale buildConfigPatch and clobber unsaved fields when the toggle fires,
  // so this handler is recreated each render to capture current state.
  const handleP2pToggle = async (val: boolean) => {
    // P0 fix: do not send the stale `config` closure snapshot directly.
    // buildConfigPatch reads current state for ALL fields and applies the
    // override, so storage/supabase fields cannot be clobbered.
    const prev = config.p2p_enabled;
    // Skip if value unchanged (guard against rapid double-toggle).
    if (val === prev || syncRestarting) return;
    setConfig((c) => ({ ...c, p2p_enabled: val }));
    try {
      await api.setConfig(
        buildConfigPatch({ p2p_enabled: val }) as unknown as Parameters<typeof api.setConfig>[0],
      );
      // The daemon only reads p2p_enabled at startup — restart so the new
      // value takes effect immediately. Show a transient status message and
      // disable the toggle while the restart is in flight to prevent queuing
      // a second restart from a rapid double-click.
      setSyncRestarting(true);
      showLimitsMsg("p2p_enabled", "Restarting sync service…", 6000);
      try {
        await restartDaemon();
        showLimitsMsg("p2p_enabled", "Sync service restarted", 2500);
      } catch (restartErr) {
        const msg =
          restartErr instanceof Error ? restartErr.message : "Restart failed — relaunch the app";
        showLimitsMsg("p2p_enabled", msg, 4000);
      } finally {
        setSyncRestarting(false);
        if (syncRestartTimerRef.current !== null) clearTimeout(syncRestartTimerRef.current);
      }
    } catch (err) {
      // Revert on set_config failure — no restart attempted.
      setConfig((c) => ({ ...c, p2p_enabled: prev }));
      const msg = err instanceof IpcError ? err.message : "Failed to update P2P setting";
      showLimitsMsg("p2p_enabled", msg, 4000);
    }
  };

  // Also NOT memoized — saveLimitsField/buildConfigPatch read live slider state
  // via closure, so the handler must be recreated each render.
  const handleWifiOnlyToggle = async (val: boolean) => {
    // P1 fix: capture `prev` BEFORE the optimistic update. saveLimitsField
    // reverts only this field on error (no full reload) and does not throw.
    const prev = syncOnWifiOnly;
    setSyncOnWifiOnly(val);
    await saveLimitsField(
      "sync_on_wifi_only",
      { sync_on_wifi_only: val },
      () => setSyncOnWifiOnly(prev),
    );
  };

  // -------------------------------------------------------------------------
  // Shortcuts — Save popup shortcut
  // -------------------------------------------------------------------------

  const handleSaveShortcut = useCallback(async () => {
    if (pendingShortcut === currentShortcut) return;
    try {
      await setPopupShortcut(pendingShortcut);
      setCurrentShortcut(pendingShortcut);
      setShortcutMsg({ text: "Saved", isError: false });
      if (shortcutTimerRef.current !== null) clearTimeout(shortcutTimerRef.current);
      shortcutTimerRef.current = setTimeout(() => setShortcutMsg(null), 2500);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to set shortcut";
      setShortcutMsg({ text: msg, isError: true });
      setPendingShortcut(currentShortcut);
      if (shortcutTimerRef.current !== null) clearTimeout(shortcutTimerRef.current);
      shortcutTimerRef.current = setTimeout(() => setShortcutMsg(null), 4000);
    }
  }, [pendingShortcut, currentShortcut]);

  // Reset the popup shortcut back to its built-in default and persist it via
  // the same IPC the manual Save uses, so the UI and registered hotkey stay
  // in sync.
  const handleResetShortcut = useCallback(async () => {
    if (currentShortcut === DEFAULT_POPUP_SHORTCUT) {
      setPendingShortcut(DEFAULT_POPUP_SHORTCUT);
      return;
    }
    setPendingShortcut(DEFAULT_POPUP_SHORTCUT);
    try {
      await setPopupShortcut(DEFAULT_POPUP_SHORTCUT);
      setCurrentShortcut(DEFAULT_POPUP_SHORTCUT);
      setShortcutMsg({ text: "Reset to default", isError: false });
      if (shortcutTimerRef.current !== null) clearTimeout(shortcutTimerRef.current);
      shortcutTimerRef.current = setTimeout(() => setShortcutMsg(null), 2500);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to reset shortcut";
      setShortcutMsg({ text: msg, isError: true });
      setPendingShortcut(currentShortcut);
      if (shortcutTimerRef.current !== null) clearTimeout(shortcutTimerRef.current);
      shortcutTimerRef.current = setTimeout(() => setShortcutMsg(null), 4000);
    }
  }, [currentShortcut]);

  // -------------------------------------------------------------------------
  // Cloud sync — Set passphrase
  // -------------------------------------------------------------------------

  const handleSetPassphrase = useCallback(async () => {
    const trimmed = passphrase.trim();
    if (!trimmed) return;
    try {
      await api.setSyncPassphrase(trimmed);
      const status = await api.getSyncStatus().catch(() => null);
      setSyncStatus(status);
      setPassphrase("");
      setPassphraseSavedMsg("Saved");
      if (passphraseTimerRef.current !== null) clearTimeout(passphraseTimerRef.current);
      passphraseTimerRef.current = setTimeout(() => setPassphraseSavedMsg(null), 2500);
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Error";
      setPassphraseSavedMsg(msg);
      if (passphraseTimerRef.current !== null) clearTimeout(passphraseTimerRef.current);
      passphraseTimerRef.current = setTimeout(() => setPassphraseSavedMsg(null), 3000);
    }
  }, [passphrase]);

  // -------------------------------------------------------------------------
  // Data — Delete all
  // -------------------------------------------------------------------------

  const handleDeleteAll = useCallback(async () => {
    setDeleteConfirm(false);
    try {
      const result = await api.deleteAll();
      setDeleteMsg({
        text: `Deleted ${result.deleted} item${result.deleted === 1 ? "" : "s"}`,
        isError: false,
      });
      if (deleteTimerRef.current !== null) clearTimeout(deleteTimerRef.current);
      deleteTimerRef.current = setTimeout(() => setDeleteMsg(null), 3000);
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Clear failed";
      setDeleteMsg({ text: msg, isError: true });
      if (deleteTimerRef.current !== null) clearTimeout(deleteTimerRef.current);
      deleteTimerRef.current = setTimeout(() => setDeleteMsg(null), 4000);
    }
  }, [deleteTimerRef]);

  // -------------------------------------------------------------------------
  // Render helpers
  // -------------------------------------------------------------------------

  // v0.5.3: inputs use global base styles from index.css; only width/padding overrides needed here
  const inputCls = [
    "w-64 px-2.5 py-1.5 text-[13px]",
    "disabled:cursor-not-allowed disabled:opacity-40",
  ].join(" ");



  const btnCls = [
    "rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
    "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
  ].join(" ");

  // Inline feedback badge for a limits field.
  function LimitsMsg({ field }: { field: string }) {
    const msg = limitsMsg[field];
    if (!msg) return null;
    const isError = msg !== "Saved";
    return (
      <span className={`text-[11px] ${isError ? "text-ide-danger" : "text-ide-success"}`}>
        {msg}
      </span>
    );
  }

  // -------------------------------------------------------------------------
  // Tab content renderers
  // -------------------------------------------------------------------------

  function renderGeneral() {
    return (
      <div className="space-y-2">
        <Panel>
          <SettingsRow label="Private mode">
            <div className="flex items-center gap-2">
              {privateModeError !== null && (
                <span className="text-[11px] text-ide-danger">{privateModeError}</span>
              )}
              <Toggle
                checked={privateMode}
                onChange={(v) => void handlePrivateMode(v)}
                disabled={offline}
              />
            </div>
          </SettingsRow>
          <SettingsRow label="Play sound on copy">
            <Toggle
              checked={prefs.playSoundOnCopy}
              onChange={(v) => {
                // P0 fix: only persist to daemon once settings are fully loaded.
                // buildConfigPatch reads hydrated slider/toggle state; calling it
                // before "ready" would push default values over the real config.
                setPrefs({ playSoundOnCopy: v });
                if (loadState === "ready") {
                  void api.setConfig(buildConfigPatch({ sound_on_copy: v }) as unknown as Parameters<typeof api.setConfig>[0]).catch(() => {
                    setPrefs({ playSoundOnCopy: !v });
                  });
                }
              }}
              disabled={offline}
            />
          </SettingsRow>
          <SettingsRow label="Show notification on copy">
            <Toggle
              checked={prefs.notifyOnCopy}
              onChange={(v) => {
                // P0 fix: same guard as sound_on_copy above.
                setPrefs({ notifyOnCopy: v });
                if (loadState === "ready") {
                  void api.setConfig(buildConfigPatch({ notify_on_copy: v }) as unknown as Parameters<typeof api.setConfig>[0]).catch(() => {
                    setPrefs({ notifyOnCopy: !v });
                  });
                }
              }}
              disabled={offline}
            />
          </SettingsRow>
          <SettingsRow label="Mask sensitive data">
            <Toggle
              checked={prefs.maskSensitive}
              onChange={(v) => setPrefs({ maskSensitive: v })}
            />
          </SettingsRow>
        </Panel>

        <SubsectionHeader
          label="Privacy & capture"
          hint="Control public-IP lookup, paste formatting, and which apps are never captured."
        />
        <Panel>
          <SettingsRow label="Discover public IP">
            <div className="flex items-center gap-1.5">
              <InfoPopover text="Allow a one-off STUN request to learn this device's public IP, shown in the device-info card. No data is sent to analytics." />
              <Toggle
                checked={collectPublicIp}
                onChange={(v) => {
                  // Mirror sound/notify: persist only once fully loaded, revert on failure.
                  setCollectPublicIp(v);
                  if (loadState === "ready") {
                    void api
                      .setConfig(buildConfigPatch({ collect_public_ip: v }) as unknown as Parameters<typeof api.setConfig>[0])
                      .catch(() => setCollectPublicIp(!v));
                  }
                }}
                disabled={offline}
              />
            </div>
          </SettingsRow>
          <SettingsRow label="Paste as plain text">
            <div className="flex items-center gap-1.5">
              <InfoPopover text="Strip rich formatting (RTF/HTML) when pasting — writes plain text only." />
              <Toggle
                checked={pasteAsPlainText}
                onChange={(v) => {
                  setPasteAsPlainText(v);
                  if (loadState === "ready") {
                    void api
                      .setConfig(buildConfigPatch({ paste_as_plain_text: v }) as unknown as Parameters<typeof api.setConfig>[0])
                      .catch(() => setPasteAsPlainText(!v));
                  }
                }}
                disabled={offline}
              />
            </div>
          </SettingsRow>
          <div className="border-b border-ide-divider/70 px-3 py-2 last:border-b-0">
            <div className="flex items-center gap-1.5">
              <span className="text-[13px] text-ide-text">Excluded apps</span>
              <InfoPopover text="Bundle IDs of apps whose clipboard is never captured, e.g. com.1password.1password (macOS)." />
            </div>
            <div className="mt-2 flex items-center gap-2">
              <input
                type="text"
                value={newExcludedApp}
                placeholder="com.example.app"
                disabled={offline}
                onChange={(e) => setNewExcludedApp(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    void addExcludedApp();
                  }
                }}
                className="flex-1 rounded-ide border border-ide-border bg-ide-bg px-2.5 py-1.5 text-[13px] text-ide-text outline-none focus:border-ide-accent focus:ring-1 focus:ring-ide-accent disabled:cursor-not-allowed disabled:opacity-40"
              />
              <button
                type="button"
                disabled={offline || newExcludedApp.trim() === ""}
                onClick={() => void addExcludedApp()}
                className="rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
              >
                Add
              </button>
            </div>
            {excludedApps.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1.5">
                {excludedApps.map((bundleId) => (
                  <span
                    key={bundleId}
                    className="inline-flex items-center gap-1 rounded-ide border border-ide-border bg-ide-bg px-2 py-1 text-[12px] text-ide-dim"
                  >
                    {bundleId}
                    <button
                      type="button"
                      aria-label={`Remove ${bundleId}`}
                      disabled={offline}
                      onClick={() => void removeExcludedApp(bundleId)}
                      className="text-ide-faint hover:text-ide-danger disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      ×
                    </button>
                  </span>
                ))}
              </div>
            )}
          </div>
        </Panel>

        <SubsectionHeader label="Daemon" />
        <Panel>
          <SettingsRow label="Version">
            <span className="text-[13px] text-ide-text">
              {offline ? "Not running" : (daemonVersion ?? "unknown")}
            </span>
          </SettingsRow>
          <SettingsRow label="Restart">
            <RestartDaemonButton onRestarted={() => setReloadKey((k) => k + 1)} />
          </SettingsRow>
        </Panel>
      </div>
    );
  }

  function renderDisplay() {
    return (
      <div className="space-y-2">
        <SubsectionHeader label="History list" />
        <Panel>
          {/* M4: split previewLines — main window has its own independent setting */}
          <SettingsRow label="Preview lines">
            <div className="flex items-center gap-1.5">
              <InfoPopover text="Number of text lines shown per clip in the main history window. Independent from the popup setting." />
              <SliderRow
                min={1}
                max={6}
                step={1}
                value={prefs.previewLinesApp}
                onChange={(v) => setPrefs({ previewLinesApp: v })}
                formatValue={(v) => String(v)}
              />
            </div>
          </SettingsRow>
          {/* M5: historySize removed — history uses lazy pagination now */}
          {/* M6: previewDelay removed — replaced by explicit Eye preview button */}
        </Panel>

        <SubsectionHeader label="Popup appearance" hint="How the popup looks when triggered." />
        <Panel>
          {/* M4: popup gets its own independent preview-lines setting */}
          <SettingsRow label="Preview lines">
            <div className="flex items-center gap-1.5">
              <InfoPopover text="Number of text lines shown per clip in the Quick-Paste popup. Independent from the main window setting." />
              <SliderRow
                min={1}
                max={6}
                step={1}
                value={prefs.previewLinesPopup}
                onChange={(v) => setPrefs({ previewLinesPopup: v })}
                formatValue={(v) => String(v)}
              />
            </div>
          </SettingsRow>
          <SettingsRow label="Image preview height">
            <div className="flex items-center gap-1.5">
              <InfoPopover text="Max image thumbnail height (1–200 px)" />
              <SliderRow
                min={1}
                max={200}
                step={1}
                value={prefs.imageMaxHeight}
                onChange={(v) => setPrefs({ imageMaxHeight: v })}
                formatValue={(v) => `${v}px`}
              />
            </div>
          </SettingsRow>
        </Panel>

        <SubsectionHeader label="Window" hint="Visual style of the application window." />
        <Panel>
          <SettingsRow label="Translucency / vibrancy">
            <div className="flex items-center gap-1.5">
              <InfoPopover text="Blur + transparency behind surfaces. Disable for solid backgrounds." />
              <Toggle
                checked={prefs.translucency ?? true}
                onChange={(v) => setPrefs({ translucency: v })}
              />
            </div>
          </SettingsRow>
        </Panel>
      </div>
    );
  }

  function renderSync() {
    return (
      <div className="space-y-2">
        {/* Status banner */}
        {syncStatus !== null && syncStatus.supabase_configured && (
          <div className="rounded-ide border border-ide-success/30 bg-ide-success/5 px-3 py-2 text-[12px] text-ide-success">
            Connected ✓
            {syncStatus.signed_in && syncStatus.email
              ? ` — signed in as ${syncStatus.email}`
              : syncStatus.signed_in
              ? " — signed in"
              : " — not signed in"}
            {syncStatus.passphrase_set ? " — passphrase set ✓" : ""}
          </div>
        )}
        {syncStatus !== null &&
          syncStatus.supabase_configured &&
          syncStatus.signed_in &&
          syncStatus.email && (
            <div className="rounded-ide border border-ide-border bg-ide-elevated px-3 py-2 text-[12px] text-ide-dim">
              <span className="font-medium text-ide-text">Signed in as {syncStatus.email}</span>
              <span className="ml-1">— All devices must use this same account to sync.</span>
            </div>
          )}

        {/* ── Local sync (P2P) ── */}
        <SubsectionHeader
          label="Local sync (P2P)"
          hint="Same network, no account needed."
        />
        <Panel>
          <SettingsRow label="Enable P2P (LAN) sync">
            <div className="flex items-center gap-2">
              <LimitsMsg field="p2p_enabled" />
              <Toggle
                checked={config.p2p_enabled}
                onChange={(v) => void handleP2pToggle(v)}
                disabled={offline || syncRestarting}
                aria-label="P2P sync"
              />
            </div>
          </SettingsRow>
          <SettingsRow label="Sync on Wi-Fi only">
            <div className="flex items-center gap-2">
              <LimitsMsg field="sync_on_wifi_only" />
              <Toggle
                checked={syncOnWifiOnly}
                onChange={(v) => void handleWifiOnlyToggle(v)}
                disabled={offline}
              />
            </div>
          </SettingsRow>
        </Panel>

        {/* ── Cloud sync (Supabase) ── */}
        <SubsectionHeader
          label="Cloud sync (Supabase)"
          hint="Syncs over the internet via your Supabase project."
        />
        <Panel>
          <SettingsRow label="Supabase URL">
            <input
              type="url"
              className={inputCls}
              placeholder="https://your-project.supabase.co"
              value={supabaseUrl}
              onChange={(e) => setSupabaseUrl(e.target.value)}
              disabled={offline}
              autoComplete="off"
              spellCheck={false}
            />
          </SettingsRow>
          <SettingsRow label="Supabase anon key">
            <div className="flex flex-col items-end gap-0.5">
              <input
                type="password"
                className={inputCls}
                placeholder={
                  syncStatus?.supabase_configured && !supabaseKey
                    ? "set ✓ (leave blank to keep)"
                    : "eyJ…"
                }
                value={supabaseKey}
                onChange={(e) => setSupabaseKey(e.target.value)}
                disabled={offline}
                autoComplete="off"
                spellCheck={false}
              />
              {syncStatus?.supabase_configured && !supabaseKey && (
                <span className="text-[11px] text-ide-success">set ✓</span>
              )}
            </div>
          </SettingsRow>
          <SettingsRow label="Relay URL">
            <div className="flex items-center gap-1.5">
              <InfoPopover text="Optional HTTP relay for store-and-forward sync when devices aren't on the same network. Leave blank to use direct P2P / cloud sync only. Saved with the cloud-sync settings." />
              <input
                type="url"
                className={inputCls}
                placeholder="https://relay.example.com"
                value={relayUrl}
                onChange={(e) => setRelayUrl(e.target.value)}
                disabled={offline}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
          </SettingsRow>
          {/* M7: "Set" button removed — passphrase saves on Enter or focus-out */}
          <SettingsRow label="Sync passphrase">
            <div className="flex flex-col items-end gap-1">
              <div className="flex items-center gap-1.5">
                <InfoPopover text="Enter the same passphrase on every device to enable encrypted sync. Saves automatically when you press Enter or move focus away." />
                <input
                  type="password"
                  className={inputCls}
                  placeholder="Shared passphrase…"
                  value={passphrase}
                  onChange={(e) => setPassphrase(e.target.value)}
                  disabled={offline}
                  autoComplete="new-password"
                  spellCheck={false}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") void handleSetPassphrase();
                  }}
                  onBlur={() => {
                    if (passphrase.trim() !== "") void handleSetPassphrase();
                  }}
                />
              </div>
              {passphraseSavedMsg !== null && (
                <span
                  className={[
                    "text-[11px]",
                    passphraseSavedMsg === "Saved" ? "text-ide-success" : "text-ide-danger",
                  ].join(" ")}
                >
                  {passphraseSavedMsg}
                </span>
              )}
            </div>
          </SettingsRow>
          {testMsg !== null && (
            <div
              className={[
                "border-t border-ide-divider px-3 py-2 text-[12px]",
                testMsg.ok ? "text-ide-success" : "text-ide-danger",
              ].join(" ")}
            >
              {testMsg.ok ? "✓ " : "✗ "}
              {testMsg.text}
            </div>
          )}
          <div className="flex items-center justify-end gap-3 border-t border-ide-divider px-3 py-2">
            {saveError !== null && (
              <span className="text-[13px] text-ide-danger">{saveError}</span>
            )}
            {savedMsg && !saveError && (
              <span className="text-[13px] text-ide-success">Saved</span>
            )}
            <button
              type="button"
              disabled={offline || testing}
              onClick={() => void handleTestConnection()}
              className={btnCls}
            >
              {testing ? "Testing…" : "Test connection"}
            </button>
            <button
              type="button"
              disabled={offline}
              onClick={() => void handleSaveConfig()}
              className={btnCls}
            >
              Save
            </button>
          </div>
        </Panel>

        {/* Sync status detail */}
        {syncStatus !== null && (
          <>
            <SubsectionHeader label="Status" />
            <Panel>
              <div className="px-3 py-2 space-y-1">
                <StatusRow label="Passphrase set" ok={syncStatus.passphrase_set} />
                <StatusRow label="Supabase configured" ok={syncStatus.supabase_configured} />
                <StatusRow label="Signed in" ok={syncStatus.signed_in} />
                <div className="flex items-center gap-2 text-[13px] text-ide-dim pt-0.5">
                  <span className="w-[140px] shrink-0">Last sync</span>
                  <span className="text-ide-text">{formatLastSync(syncStatus.last_sync_ms)}</span>
                </div>
              </div>
            </Panel>
          </>
        )}
      </div>
    );
  }

  function renderShortcuts() {
    return (
      <div className="space-y-2">
        <Panel>
          <SettingsRow label="Open popup">
            <div className="flex flex-col items-end gap-1">
              <div className="flex items-center gap-2">
                <InfoPopover text="Click then press a combo. OS-reserved keys (Cmd+Space etc.) cannot be overridden." />
                <ShortcutCapture
                  value={pendingShortcut}
                  onChange={setPendingShortcut}
                />
                <button
                  type="button"
                  aria-label="Reset shortcut to default"
                  title={`Reset to default (${DEFAULT_POPUP_SHORTCUT})`}
                  disabled={
                    currentShortcut === DEFAULT_POPUP_SHORTCUT &&
                    pendingShortcut === DEFAULT_POPUP_SHORTCUT
                  }
                  onClick={() => void handleResetShortcut()}
                  className="flex h-7 w-7 items-center justify-center rounded-ide border border-ide-border bg-ide-elevated text-ide-dim hover:bg-ide-hover hover:text-ide-text disabled:cursor-not-allowed disabled:opacity-40 transition-colors"
                >
                  <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <path d="M2.5 8a5.5 5.5 0 1 1 1.6 3.9" />
                    <path d="M2.5 12v-3.2h3.2" />
                  </svg>
                </button>
                <button
                  type="button"
                  disabled={pendingShortcut === currentShortcut}
                  onClick={() => void handleSaveShortcut()}
                  className={btnCls}
                >
                  Save
                </button>
              </div>
              {shortcutMsg !== null && (
                <span
                  className={[
                    "text-[11px]",
                    shortcutMsg.isError ? "text-ide-danger" : "text-ide-success",
                  ].join(" ")}
                >
                  {shortcutMsg.text}
                </span>
              )}
            </div>
          </SettingsRow>
        </Panel>
      </div>
    );
  }

  function renderStorage() {
    // Helper: render a stepped slider row with inline feedback badge.
    // M9: LimitSliderRow now uses the unified SliderRow (index-based 0…steps.length-1).
    // onRelease fires only on mouse-up/touch-end to avoid hammering the IPC on drag.
    function LimitSliderRow<T extends number>({
      label,
      field,
      steps,
      labels,
      value,
      onChange,
      onRelease,
    }: {
      label: string;
      field: string;
      steps: readonly T[];
      labels: readonly string[];
      value: T;
      onChange: (v: T) => void;
      onRelease: (v: T) => void;
    }) {
      const maxIdx = steps.length - 1;
      const idx = steps.indexOf(value);
      const safeIdx = idx < 0 ? 0 : idx;
      return (
        <SettingsRow label={label}>
          <div className="flex items-center gap-2">
            <SliderRow
              min={0}
              max={maxIdx}
              step={1}
              value={safeIdx}
              disabled={offline}
              onChange={(i) => onChange(steps[Math.min(Math.max(i, 0), maxIdx)] as T)}
              onRelease={(i) => onRelease(steps[Math.min(Math.max(i, 0), maxIdx)] as T)}
              formatValue={(i) => labels[Math.min(Math.max(i, 0), maxIdx)] ?? String(i)}
            />
            <LimitsMsg field={field} />
          </div>
        </SettingsRow>
      );
    }

    return (
      <div className="space-y-2">
        <Panel>
          <LimitSliderRow
            label="Max clip text size"
            field="max_text_size_bytes"
            steps={TEXT_SIZE_STEPS_BYTES as unknown as readonly number[]}
            labels={TEXT_SIZE_LABELS}
            value={maxTextBytes}
            onChange={(v) => setMaxTextBytes(v)}
            onRelease={(v) => {
              // P1 fix: capture prev before optimistic update (onChange already fired);
              // revert only this field on error, not the full reload.
              const prev = maxTextBytes;
              setMaxTextBytes(v);
              void saveLimitsField("max_text_size_bytes", { max_text_size_bytes: v }, () => setMaxTextBytes(prev));
            }}
          />
          <LimitSliderRow
            label="Max clip image size"
            field="max_image_size_bytes"
            steps={IMAGE_SIZE_STEPS_BYTES as unknown as readonly number[]}
            labels={IMAGE_SIZE_LABELS}
            value={maxImageBytes}
            onChange={(v) => setMaxImageBytes(v)}
            onRelease={(v) => {
              const prev = maxImageBytes;
              setMaxImageBytes(v);
              void saveLimitsField("max_image_size_bytes", { max_image_size_bytes: v }, () => setMaxImageBytes(prev));
            }}
          />
          <LimitSliderRow
            label="Max clip file size"
            field="max_file_size_bytes"
            steps={FILE_SIZE_STEPS_BYTES as unknown as readonly number[]}
            labels={FILE_SIZE_LABELS}
            value={maxFileBytes}
            onChange={(v) => setMaxFileBytes(v)}
            onRelease={(v) => {
              const prev = maxFileBytes;
              setMaxFileBytes(v);
              void saveLimitsField("max_file_size_bytes", { max_file_size_bytes: v }, () => setMaxFileBytes(prev));
            }}
          />
          <div className="border-b border-ide-divider/70 px-3 pb-2 text-[11px] text-ide-faint">
            Files over ~8&nbsp;MB are kept locally but won&apos;t sync over P2P/cloud — they&apos;re skipped with a warning.
          </div>
          <LimitSliderRow
            label="Local storage limit"
            field="storage_quota_bytes"
            steps={QUOTA_STEPS_BYTES as unknown as readonly number[]}
            labels={QUOTA_LABELS}
            value={quotaBytes}
            onChange={(v) => setQuotaBytes(v)}
            onRelease={(v) => {
              const prev = quotaBytes;
              setQuotaBytes(v);
              void saveLimitsField("storage_quota_bytes", { storage_quota_bytes: v }, () => setQuotaBytes(prev));
            }}
          />
          <LimitSliderRow
            label="Sensitive auto-wipe"
            field="sensitive_ttl_secs"
            steps={SENSITIVE_TTL_STEPS as unknown as readonly number[]}
            labels={SENSITIVE_TTL_LABELS}
            value={sensitiveTtlSecs}
            onChange={(v) => setSensitiveTtlSecs(v)}
            onRelease={(v) => {
              const prev = sensitiveTtlSecs;
              setSensitiveTtlSecs(v);
              void saveLimitsField("sensitive_ttl_secs", { sensitive_ttl_secs: v }, () => setSensitiveTtlSecs(prev));
            }}
          />
          <SettingsRow label="Image quality (1–100)">
            <div className="flex items-center gap-2">
              <SliderRow
                min={1}
                max={100}
                step={1}
                value={imageQuality}
                onChange={(v) => setImageQuality(v)}
                onRelease={(v) => {
                  // Autosave on commit (mouse-up / touch-end / key-up), matching the
                  // neighbouring limit sliders — no dedicated Save button.
                  const prev = imageQuality;
                  void saveLimitsField("image_quality", { image_quality: v }, () => setImageQuality(prev));
                }}
                formatValue={(v) => String(v)}
              />
              <LimitsMsg field="image_quality" />
            </div>
          </SettingsRow>
        </Panel>

        <SubsectionHeader label="Data" />
        <Panel>
          <SettingsRow label="Clear clipboard history">
            <div className="flex items-center gap-3">
              {deleteMsg !== null && (
                <span
                  className={[
                    "text-[13px]",
                    deleteMsg.isError ? "text-ide-danger" : "text-ide-dim",
                  ].join(" ")}
                >
                  {deleteMsg.text}
                </span>
              )}
              {deleteConfirm ? (
                <span className="flex items-center gap-1.5 text-[13px]">
                  <span className="text-ide-dim">Delete all history?</span>
                  <button
                    type="button"
                    onClick={() => void handleDeleteAll()}
                    className="rounded-ide border border-ide-danger/50 bg-ide-elevated px-2.5 py-1 text-[13px] text-ide-danger hover:bg-ide-hover"
                  >
                    Yes
                  </button>
                  <button
                    type="button"
                    onClick={() => setDeleteConfirm(false)}
                    className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[13px] text-ide-dim hover:bg-ide-hover"
                  >
                    No
                  </button>
                </span>
              ) : (
                <button
                  type="button"
                  disabled={offline}
                  onClick={() => setDeleteConfirm(true)}
                  className={[
                    "rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-danger",
                    "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
                  ].join(" ")}
                >
                  Clear history…
                </button>
              )}
            </div>
          </SettingsRow>
        </Panel>
      </div>
    );
  }

  function renderAdvanced() {
    return (
      <div className="space-y-2">
        <div className="rounded-ide border border-ide-border bg-ide-elevated px-3 py-3 text-[13px] text-ide-dim">
          Advanced daemon and storage limits will appear here in a future release.
        </div>
      </div>
    );
  }

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  return (
    <ViewShell title="Settings">
      {/* Stale-daemon banner */}
      {staleDaemon !== null && (
        <div className="mb-4 flex items-start justify-between gap-3 rounded-ide border border-ide-warning/40 bg-ide-warning/5 px-3 py-2 text-[13px] text-ide-warning">
          <span>
            A previous CopyPaste daemon is still running after an update
            {staleDaemon !== "unknown" ? ` (build ${staleDaemon})` : ""}. Restart
            it to use the latest version.
          </span>
          <RestartDaemonButton onRestarted={() => setReloadKey((k) => k + 1)} />
        </div>
      )}

      {/* Offline banner */}
      {loadState === "offline" && (
        <div className="mb-4 flex items-center justify-between gap-3 rounded-ide-lg border border-ide-border bg-ide-elevated px-3 py-2 text-[13px] text-ide-dim shadow-ide-xs">
          <span>Daemon not running — clipboard sync paused.</span>
          <div className="flex shrink-0 items-center gap-2">
            <RestartDaemonButton
              label="Restart daemon"
              onRestarted={() => setReloadKey((k) => k + 1)}
            />
            <button
              type="button"
              onClick={() => setReloadKey((k) => k + 1)}
              className="shrink-0 rounded-ide border border-ide-border bg-ide-panel px-2.5 py-1 text-[12px] text-ide-text hover:bg-ide-raised hover:text-ide-text shadow-ide-xs"
            >
              Retry
            </button>
          </div>
        </div>
      )}

      {/* Degraded banner */}
      {degraded && (
        <div className="mb-4 flex items-start justify-between gap-3 rounded-ide border border-ide-warning/40 bg-ide-warning/5 px-3 py-2 text-[13px] text-ide-warning">
          <span>
            Clipboard database unavailable
            {degradedReason ? ` (${degradedReason})` : ""} — its key no longer
            matches. Open History to reset the database and recover.
          </span>
          <div className="flex shrink-0 items-center gap-2">
            <button
              type="button"
              onClick={() => setReloadKey((k) => k + 1)}
              className={[
                "rounded-ide border border-ide-warning/40 bg-ide-panel px-2.5 py-1 text-[12px] text-ide-warning",
                "hover:bg-ide-hover",
              ].join(" ")}
            >
              Retry
            </button>
            <RestartDaemonButton onRestarted={() => setReloadKey((k) => k + 1)} />
          </div>
        </div>
      )}

      {/* Loading */}
      {loadState === "loading" && (
        <div className="flex h-full items-center justify-center text-[13px] text-ide-dim">
          Loading…
        </div>
      )}

      {loadState !== "loading" && (
        <div className="mx-auto max-w-xl">
          <TabBar active={activeTab} onChange={setActiveTab} />
          {activeTab === "general"   && renderGeneral()}
          {activeTab === "display"   && renderDisplay()}
          {activeTab === "sync"      && renderSync()}
          {activeTab === "shortcuts" && renderShortcuts()}
          {activeTab === "storage"   && renderStorage()}
          {activeTab === "advanced"  && renderAdvanced()}
        </div>
      )}
    </ViewShell>
  );
}
