import { useCallback, useEffect, useRef, useState } from "react";
import { ViewShell } from "../components/ViewShell";
import {
  api,
  IpcError,
  appVersion,
  getPopupShortcut,
  setPopupShortcut,
  detectStaleDaemonFromStatus,
  type AppSettings,
  type SyncStatus,
  type DaemonStatus,
} from "../lib/ipc";
import { RestartDaemonButton } from "../components/RestartDaemonButton";
import { useUI } from "../store";

// ---------------------------------------------------------------------------
// Toggle — iOS-style switch using ide tokens
// ---------------------------------------------------------------------------

function Toggle({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean;
  onChange: (val: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={[
        "relative inline-flex h-[18px] w-[32px] shrink-0 cursor-pointer items-center rounded-full",
        "border transition-colors duration-150 focus:outline-none focus:ring-1 focus:ring-ide-accent focus:ring-offset-1 focus:ring-offset-ide-bg",
        "disabled:cursor-not-allowed disabled:opacity-40",
        checked
          ? "border-ide-accent bg-ide-accent"
          : "border-ide-border bg-ide-elevated",
      ].join(" ")}
    >
      <span
        className={[
          "inline-block h-[12px] w-[12px] rounded-full bg-white shadow-sm transition-transform duration-150",
          checked ? "translate-x-[16px]" : "translate-x-[2px]",
        ].join(" ")}
      />
    </button>
  );
}

// ---------------------------------------------------------------------------
// Shared layout primitives
// ---------------------------------------------------------------------------

function SectionHeader({ label }: { label: string }) {
  return (
    <div className="mb-2 mt-8 first:mt-0 px-0 text-[11px] uppercase tracking-wide text-ide-faint">
      {label}
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
    <div className="flex min-h-[34px] items-center justify-between border-b border-ide-divider px-3 py-1.5 last:border-b-0">
      {/* W4-3: fixed min-width on label column prevents wrapping on narrow labels */}
      <span className="min-w-[160px] shrink-0 text-[13px] text-ide-dim">{label}</span>
      <div className="flex items-center gap-2">{children}</div>
    </div>
  );
}

function Panel({ children }: { children: React.ReactNode }) {
  return (
    <div className="overflow-hidden rounded-ide border border-ide-border bg-ide-panel">
      {children}
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
// Both sliders in the Display section use this component so columns align.
// ---------------------------------------------------------------------------

function SliderRow({
  min,
  max,
  step,
  value,
  onChange,
  formatValue,
}: {
  min: number;
  max: number;
  step: number;
  value: number;
  onChange: (v: number) => void;
  /** Format the numeric value for the right-hand value label. */
  formatValue: (v: number) => string;
}) {
  return (
    // Grid: slider expands to fill, value label is fixed 52px right-aligned.
    <div className="flex items-center gap-2">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-28 accent-ide-accent"
      />
      {/* Fixed width + text-right keeps all value labels in the same column. */}
      <span className="w-[52px] text-right text-[13px] text-ide-text">
        {formatValue(value)}
      </span>
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

/** Convert bytes to MB, rounded to one decimal place. */
function bytesToMb(bytes: number): number {
  return Math.round((bytes / 1_000_000) * 10) / 10;
}

/** Convert MB back to bytes (integer). */
function mbToBytes(mb: number): number {
  return Math.round(mb * 1_000_000);
}

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

  // General
  const [privateMode, setPrivateMode] = useState(false);

  // Sync / cloud config
  const [config, setConfig] = useState<AppSettings>({
    p2p_enabled: false,
    supabase_url: null,
    supabase_anon_key: null,
  });
  const [supabaseUrl, setSupabaseUrl] = useState("");
  const [supabaseKey, setSupabaseKey] = useState("");
  const [savedMsg, setSavedMsg] = useState(false);
  const [testMsg, setTestMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const [testing, setTesting] = useState(false);

  // Cloud sync passphrase
  const [passphrase, setPassphrase] = useState("");
  const [passphraseSavedMsg, setPassphraseSavedMsg] = useState<string | null>(null);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);

  // Shortcuts
  const [currentShortcut, setCurrentShortcut] = useState("CmdOrCtrl+Shift+V");
  const [pendingShortcut, setPendingShortcut] = useState("CmdOrCtrl+Shift+V");
  const [shortcutMsg, setShortcutMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const shortcutTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Storage / Limits — fields from AppConfig surfaced via get_config / set_config.
  // MB representations; converted to/from raw bytes before IPC.
  const [maxTextMb, setMaxTextMb] = useState(1);
  const [maxImageMb, setMaxImageMb] = useState(25);
  const [maxFileMb, setMaxFileMb] = useState(100);
  const [quotaMb, setQuotaMb] = useState(500);
  const [historyLimit, setHistoryLimit] = useState(1000);
  const [sensitiveTtlSecs, setSensitiveTtlSecs] = useState(30);
  const [imageQuality, setImageQuality] = useState(80);
  // Per-field save feedback: key = field name, value = error or "Saved" / null.
  const [limitsMsg, setLimitsMsg] = useState<Record<string, string | null>>({});
  const limitsMsgTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({});

  // Sync parity — p2p toggle + wifi-only (supabase_email/password not in AppConfig → deferred)
  const [syncOnWifiOnly, setSyncOnWifiOnly] = useState(false);

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
          p2p_enabled: rawCfg.p2p_enabled ?? false,
          supabase_url: rawCfg.supabase_url ?? null,
          supabase_anon_key: rawCfg.supabase_anon_key ?? null,
        });

        // Prefill Supabase URL — prefer stored config, fall back to sync_status.
        setSupabaseUrl(rawCfg.supabase_url ?? syncSt?.supabase_url ?? "");
        setSupabaseKey(rawCfg.supabase_anon_key ?? "");
        setSyncStatus(syncSt);

        // Storage / Limits — convert raw bytes to MB for display.
        setMaxTextMb(bytesToMb(rawCfg.max_text_size_bytes ?? 1_000_000));
        setMaxImageMb(bytesToMb(rawCfg.max_image_size_bytes ?? 25_000_000));
        setMaxFileMb(bytesToMb(rawCfg.max_file_size_bytes ?? 100_000_000));
        setQuotaMb(bytesToMb(rawCfg.storage_quota_bytes ?? 500_000_000));
        setHistoryLimit(rawCfg.history_limit ?? 1000);
        setSensitiveTtlSecs(rawCfg.sensitive_ttl_secs ?? 30);
        setImageQuality(rawCfg.image_quality ?? 80);

        // Sync parity
        setSyncOnWifiOnly(rawCfg.sync_on_wifi_only ?? false);

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
    };
  }, [reloadKey]);

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

  // Build the full AppSettings patch for set_config, merging current config
  // with any updated limits fields. Converts MB back to raw bytes.
  function buildConfigPatch(overrides: Partial<AppSettings>): AppSettings {
    return {
      p2p_enabled: config.p2p_enabled,
      supabase_url: supabaseUrl.trim() || null,
      supabase_anon_key: supabaseKey.trim() || null,
      max_text_size_bytes: mbToBytes(maxTextMb),
      max_image_size_bytes: mbToBytes(maxImageMb),
      max_file_size_bytes: mbToBytes(maxFileMb),
      storage_quota_bytes: mbToBytes(quotaMb),
      history_limit: historyLimit,
      sensitive_ttl_secs: sensitiveTtlSecs,
      image_quality: imageQuality,
      sync_on_wifi_only: syncOnWifiOnly,
      ...overrides,
    };
  }

  async function saveLimitsField(field: string, patch: Partial<AppSettings>) {
    try {
      await api.setConfig(buildConfigPatch(patch) as unknown as Parameters<typeof api.setConfig>[0]);
      showLimitsMsg(field, "Saved", 2000);
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Save failed";
      showLimitsMsg(field, msg, 4000);
      // Revert local state to what the daemon had before.
      setReloadKey((k) => k + 1);
    }
  }

  // -------------------------------------------------------------------------
  // General — Private mode
  // -------------------------------------------------------------------------

  const handlePrivateMode = useCallback(
    async (val: boolean) => {
      setPrivateMode(val);
      setPrivateModeError(null);
      try {
        await api.setPrivateMode(val);
      } catch (err) {
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
    const next: AppSettings = {
      p2p_enabled: config.p2p_enabled,
      supabase_url: supabaseUrl.trim() || null,
      supabase_anon_key: supabaseKey.trim() || null,
    };
    setSaveError(null);
    try {
      await api.setConfig(next);
      setConfig(next);
      setSavedMsg(true);
      if (savedTimerRef.current !== null) clearTimeout(savedTimerRef.current);
      savedTimerRef.current = setTimeout(() => setSavedMsg(false), 2500);
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Save failed";
      setSaveError(msg);
      if (saveErrTimer.current !== null) clearTimeout(saveErrTimer.current);
      saveErrTimer.current = setTimeout(() => setSaveError(null), 3500);
    }
  }, [config.p2p_enabled, supabaseUrl, supabaseKey, saveErrTimer]);

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

  const handleP2pToggle = useCallback(
    async (val: boolean) => {
      const prev = config.p2p_enabled;
      setConfig((c) => ({ ...c, p2p_enabled: val }));
      try {
        await api.setConfig({ ...config, p2p_enabled: val });
      } catch (err) {
        // Revert on failure
        setConfig((c) => ({ ...c, p2p_enabled: prev }));
        const msg = err instanceof IpcError ? err.message : "Failed to update P2P setting";
        showLimitsMsg("p2p_enabled", msg, 4000);
      }
    },
    [config]
  );

  const handleWifiOnlyToggle = useCallback(
    async (val: boolean) => {
      const prev = syncOnWifiOnly;
      setSyncOnWifiOnly(val);
      try {
        await saveLimitsField("sync_on_wifi_only", { sync_on_wifi_only: val });
      } catch {
        // saveLimitsField already handles revert + error display
        setSyncOnWifiOnly(prev);
      }
    },
    // saveLimitsField is stable (defined inline) — only syncOnWifiOnly matters
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [syncOnWifiOnly]
  );

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

  const inputCls = [
    "w-64 rounded-ide border border-ide-border bg-ide-bg px-2.5 py-1.5 text-[13px] text-ide-text",
    "outline-none focus:border-ide-accent placeholder:text-ide-faint",
    "disabled:cursor-not-allowed disabled:opacity-40",
  ].join(" ");

  const numberInputCls = [
    "w-20 rounded-ide border border-ide-border bg-ide-bg px-2 py-1",
    "text-[13px] text-ide-text outline-none focus:border-ide-accent",
    "disabled:cursor-not-allowed disabled:opacity-40",
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
        <div className="mb-4 flex items-center justify-between gap-3 rounded-ide border border-ide-border bg-ide-elevated px-3 py-2 text-[13px] text-ide-dim">
          <span>Daemon not running — clipboard sync paused.</span>
          <div className="flex shrink-0 items-center gap-2">
            <RestartDaemonButton
              label="Restart daemon"
              onRestarted={() => setReloadKey((k) => k + 1)}
            />
            <button
              type="button"
              onClick={() => setReloadKey((k) => k + 1)}
              className={[
                "shrink-0 rounded-ide border border-ide-border bg-ide-panel px-2.5 py-1 text-[12px] text-ide-text",
                "hover:bg-ide-hover",
              ].join(" ")}
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
        <div className="mx-auto max-w-xl space-y-2">

          {/* ── General ──────────────────────────────────────────────────── */}
          <SectionHeader label="General" />
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
          </Panel>

          {/* ── Daemon ───────────────────────────────────────────────────── */}
          <SectionHeader label="Daemon" />
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

          {/* ── Display ──────────────────────────────────────────────────── */}
          <SectionHeader label="Display" />
          <Panel>
            {/* W4-2: both slider rows use SliderRow for aligned label/slider/value grid */}
            <SettingsRow label="Preview lines">
              <SliderRow
                min={1}
                max={6}
                step={1}
                value={prefs.previewLines}
                onChange={(v) => setPrefs({ previewLines: v })}
                formatValue={(v) => String(v)}
              />
            </SettingsRow>
            <SettingsRow label="Row height">
              <SliderRow
                min={24}
                max={64}
                step={4}
                value={prefs.previewSize}
                onChange={(v) => setPrefs({ previewSize: v })}
                formatValue={(v) => `${v}px`}
              />
            </SettingsRow>
            <SettingsRow label="Image thumbnail height">
              <div className="flex flex-col items-end gap-0.5">
                <SliderRow
                  min={1}
                  max={200}
                  step={1}
                  value={prefs.imageMaxHeight}
                  onChange={(v) => setPrefs({ imageMaxHeight: v })}
                  formatValue={(v) => `${v}px`}
                />
                <span className="text-[11px] text-ide-faint">
                  Max image preview height (1–200 px).
                </span>
              </div>
            </SettingsRow>
            <SettingsRow label="History size">
              <div className="flex flex-col items-end gap-0.5">
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    min={1}
                    max={999}
                    step={1}
                    value={prefs.historySize}
                    onChange={(e) => {
                      const v = Math.max(1, Math.min(999, Number(e.target.value) || 1));
                      setPrefs({ historySize: v });
                    }}
                    className={numberInputCls}
                  />
                  <span className="text-[13px] text-ide-dim">items</span>
                </div>
                <span className="text-[11px] text-ide-faint">
                  Items displayed (1–999).
                </span>
              </div>
            </SettingsRow>
            <SettingsRow label="Preview delay">
              <div className="flex flex-col items-end gap-0.5">
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    min={200}
                    max={100000}
                    step={100}
                    value={prefs.previewDelay}
                    onChange={(e) => {
                      const v = Math.max(200, Math.min(100000, Number(e.target.value) || 1500));
                      setPrefs({ previewDelay: v });
                    }}
                    className={`${numberInputCls} w-24`}
                  />
                  <span className="text-[13px] text-ide-dim">ms</span>
                </div>
                <span className="text-[11px] text-ide-faint">
                  Hover delay before large preview (200–100 000 ms).
                </span>
              </div>
            </SettingsRow>
            <SettingsRow label="Mask sensitive data">
              <Toggle
                checked={prefs.maskSensitive}
                onChange={(v) => setPrefs({ maskSensitive: v })}
              />
            </SettingsRow>
          </Panel>

          {/* ── Notifications (W4-6) ────────────────────────────────── */}
          {/* Maccy-style copy feedback toggles. Both default off (Maccy default).
              Persisted to localStorage via UIPrefs — no daemon involvement. */}
          <SectionHeader label="Notifications" />
          <Panel>
            <SettingsRow label="Play sound on copy">
              <Toggle
                checked={prefs.playSoundOnCopy}
                onChange={(v) => setPrefs({ playSoundOnCopy: v })}
              />
            </SettingsRow>
            <SettingsRow label="Show notification on copy">
              <Toggle
                checked={prefs.notifyOnCopy}
                onChange={(v) => setPrefs({ notifyOnCopy: v })}
              />
            </SettingsRow>
          </Panel>

          {/* ── Shortcuts ────────────────────────────────────────────────── */}
          {/* W4-3: SettingsRow now has min-w-[160px] on the label so "Open popup"
              never wraps; all label columns align with other rows. */}
          <SectionHeader label="Shortcuts" />
          <Panel>
            <SettingsRow label="Open popup">
              <div className="flex flex-col items-end gap-1">
                <div className="flex items-center gap-2">
                  <ShortcutCapture
                    value={pendingShortcut}
                    onChange={setPendingShortcut}
                  />
                  <button
                    type="button"
                    disabled={pendingShortcut === currentShortcut}
                    onClick={() => void handleSaveShortcut()}
                    className={[
                      "rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
                      "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
                    ].join(" ")}
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
                {/* W4-1: shortened help text */}
                <span className="text-[11px] text-ide-faint">
                  Click then press a combo. OS-reserved keys (Cmd+Space etc.) cannot be overridden.
                </span>
              </div>
            </SettingsRow>
          </Panel>

          {/* ── Sync ─────────────────────────────────────────────────────── */}
          <SectionHeader label="Sync" />
          {syncStatus !== null && syncStatus.supabase_configured && (
            <div className="mb-1 rounded-ide border border-ide-success/30 bg-ide-success/5 px-3 py-2 text-[12px] text-ide-success">
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
              <div className="mb-1 rounded-ide border border-ide-border bg-ide-elevated px-3 py-2 text-[12px] text-ide-dim">
                <span className="font-medium text-ide-text">
                  Signed in as {syncStatus.email}
                </span>
                <span className="ml-1">
                  — All devices must use this same account to sync.
                </span>
              </div>
            )}
          <Panel>
            {/* Sync parity: P2P toggle (Android already has this) */}
            <SettingsRow label="Enable P2P (LAN) sync">
              <div className="flex items-center gap-2">
                <LimitsMsg field="p2p_enabled" />
                <Toggle
                  checked={config.p2p_enabled}
                  onChange={(v) => void handleP2pToggle(v)}
                  disabled={offline}
                />
              </div>
            </SettingsRow>
            {/* Sync parity: wifi-only toggle */}
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
                className={[
                  "rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
                  "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
                ].join(" ")}
              >
                {testing ? "Testing…" : "Test connection"}
              </button>
              <button
                type="button"
                disabled={offline}
                onClick={() => void handleSaveConfig()}
                className={[
                  "rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
                  "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
                ].join(" ")}
              >
                Save
              </button>
            </div>
          </Panel>

          {/* ── Cloud Sync ───────────────────────────────────────────────── */}
          <SectionHeader label="Cloud Sync" />
          <Panel>
            <SettingsRow label="Sync passphrase">
              <div className="flex flex-col items-end gap-1">
                <div className="flex items-center gap-2">
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
                  />
                  <button
                    type="button"
                    disabled={offline || passphrase.trim() === ""}
                    onClick={() => void handleSetPassphrase()}
                    className={[
                      "rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
                      "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
                    ].join(" ")}
                  >
                    Set
                  </button>
                </div>
                {passphraseSavedMsg !== null && (
                  <span
                    className={[
                      "text-[11px]",
                      passphraseSavedMsg === "Saved"
                        ? "text-ide-success"
                        : "text-ide-danger",
                    ].join(" ")}
                  >
                    {passphraseSavedMsg}
                  </span>
                )}
                <span className="text-[11px] text-ide-faint">
                  Same passphrase on every device to sync.
                </span>
              </div>
            </SettingsRow>
            <div className="border-t border-ide-divider px-3 py-2 space-y-1">
              <div className="text-[11px] uppercase tracking-wide text-ide-faint mb-1">
                Status
              </div>
              {syncStatus === null ? (
                <div className="text-[13px] text-ide-dim">
                  {offline
                    ? "Unavailable (daemon offline)"
                    : "Not available in this daemon build"}
                </div>
              ) : (
                <div className="space-y-0.5">
                  <StatusRow label="Passphrase set" ok={syncStatus.passphrase_set} />
                  <StatusRow label="Supabase configured" ok={syncStatus.supabase_configured} />
                  <StatusRow label="Signed in" ok={syncStatus.signed_in} />
                  <div className="flex items-center gap-2 text-[13px] text-ide-dim pt-0.5">
                    <span className="w-[140px] shrink-0">Last sync</span>
                    <span className="text-ide-text">
                      {formatLastSync(syncStatus.last_sync_ms)}
                    </span>
                  </div>
                </div>
              )}
            </div>
          </Panel>

          {/* ── Storage / Limits ─────────────────────────────────────────── */}
          {/* All fields read from get_config and written via set_config on commit.
              Byte fields are displayed in MB; converted back to raw bytes before
              sending to the daemon. */}
          <SectionHeader label="Storage / Limits" />
          <Panel>
            <SettingsRow label="Max clip text size (MB)">
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  min={0.1}
                  max={100}
                  step={0.1}
                  value={maxTextMb}
                  onChange={(e) => setMaxTextMb(Math.max(0.1, Number(e.target.value) || 1))}
                  onBlur={() => void saveLimitsField("max_text_size_bytes", { max_text_size_bytes: mbToBytes(maxTextMb) })}
                  className={`${numberInputCls} w-24`}
                  disabled={offline}
                />
                <LimitsMsg field="max_text_size_bytes" />
              </div>
            </SettingsRow>
            <SettingsRow label="Max clip image size (MB)">
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  min={1}
                  max={500}
                  step={1}
                  value={maxImageMb}
                  onChange={(e) => setMaxImageMb(Math.max(1, Number(e.target.value) || 25))}
                  onBlur={() => void saveLimitsField("max_image_size_bytes", { max_image_size_bytes: mbToBytes(maxImageMb) })}
                  className={`${numberInputCls} w-24`}
                  disabled={offline}
                />
                <LimitsMsg field="max_image_size_bytes" />
              </div>
            </SettingsRow>
            <SettingsRow label="Max clip file size (MB)">
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  min={1}
                  max={2000}
                  step={1}
                  value={maxFileMb}
                  onChange={(e) => setMaxFileMb(Math.max(1, Number(e.target.value) || 100))}
                  onBlur={() => void saveLimitsField("max_file_size_bytes", { max_file_size_bytes: mbToBytes(maxFileMb) })}
                  className={`${numberInputCls} w-24`}
                  disabled={offline}
                />
                <LimitsMsg field="max_file_size_bytes" />
              </div>
            </SettingsRow>
            <SettingsRow label="Local storage limit (MB)">
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  min={50}
                  max={50000}
                  step={50}
                  value={quotaMb}
                  onChange={(e) => setQuotaMb(Math.max(50, Number(e.target.value) || 500))}
                  onBlur={() => void saveLimitsField("storage_quota_bytes", { storage_quota_bytes: mbToBytes(quotaMb) })}
                  className={`${numberInputCls} w-24`}
                  disabled={offline}
                />
                <LimitsMsg field="storage_quota_bytes" />
              </div>
            </SettingsRow>
            <SettingsRow label="Max stored items">
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  min={1}
                  max={100000}
                  step={1}
                  value={historyLimit}
                  onChange={(e) => setHistoryLimit(Math.max(1, Number(e.target.value) || 1000))}
                  onBlur={() => void saveLimitsField("history_limit", { history_limit: historyLimit })}
                  className={`${numberInputCls} w-24`}
                  disabled={offline}
                />
                <LimitsMsg field="history_limit" />
              </div>
            </SettingsRow>
            <SettingsRow label="Sensitive auto-wipe delay (s)">
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  min={1}
                  max={86400}
                  step={1}
                  value={sensitiveTtlSecs}
                  onChange={(e) => setSensitiveTtlSecs(Math.max(1, Number(e.target.value) || 30))}
                  onBlur={() => void saveLimitsField("sensitive_ttl_secs", { sensitive_ttl_secs: sensitiveTtlSecs })}
                  className={`${numberInputCls} w-24`}
                  disabled={offline}
                />
                <LimitsMsg field="sensitive_ttl_secs" />
              </div>
            </SettingsRow>
            <SettingsRow label="Image quality (1–100)">
              <div className="flex items-center gap-2">
                <SliderRow
                  min={1}
                  max={100}
                  step={1}
                  value={imageQuality}
                  onChange={(v) => setImageQuality(v)}
                  formatValue={(v) => String(v)}
                />
                <LimitsMsg field="image_quality" />
              </div>
            </SettingsRow>
            {/* Save button for image quality (slider — no onBlur like inputs) */}
            <div className="flex justify-end border-t border-ide-divider px-3 py-2">
              <button
                type="button"
                disabled={offline}
                onClick={() => void saveLimitsField("image_quality", { image_quality: imageQuality })}
                className={[
                  "rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
                  "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
                ].join(" ")}
              >
                Save image quality
              </button>
            </div>
          </Panel>

          {/* ── Data ─────────────────────────────────────────────────────── */}
          <SectionHeader label="Data" />
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
      )}
    </ViewShell>
  );
}
