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
    // Fix #9: more vertical breathing room between sections
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
      <span className="text-[13px] text-ide-dim">{label}</span>
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

  // Normalize the key name to what Tauri/tao expects.
  // Always derive the key from the PHYSICAL key (e.code), never e.key, so the
  // shortcut is keyboard-layout-independent: the physical Q key records "Q"
  // whether the active layout is English, Ukrainian, etc. e.key would yield the
  // localized character (Cyrillic "й") or, with Option, a composed glyph ("Œ").
  let key: string;
  if (e.code.startsWith("Key")) {
    key = e.code.slice(3); // "KeyQ" → "Q"
  } else if (e.code.startsWith("Digit")) {
    key = e.code.slice(5); // "Digit1" → "1"
  } else {
    key = e.code || e.key; // "Space", "Enter", "ArrowUp", "F5", ...
  }

  if (key.length === 1) {
    key = key.toUpperCase();
  } else {
    // Map browser key names to Tauri accelerator key names.
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
  // Select each field with its own selector returning a STABLE reference.
  // Returning a fresh object literal `(s) => ({ prefs, setPrefs })` from a
  // single selector makes the store snapshot unstable on every render, which
  // under Zustand v5 + React's useSyncExternalStore throws/loops — that throw,
  // with no error boundary, was blanking the whole window when opening Settings.
  const prefs = useUI((s) => s.prefs);
  const setPrefs = useUI((s) => s.setPrefs);

  // General
  const [privateMode, setPrivateMode] = useState(false);

  // Sync
  const [config, setConfig] = useState<AppSettings>({
    p2p_enabled: false,
    supabase_url: null,
    supabase_anon_key: null,
  });
  const [supabaseUrl, setSupabaseUrl] = useState("");
  const [supabaseKey, setSupabaseKey] = useState("");
  const [savedMsg, setSavedMsg] = useState(false);
  // Cloud connection test result (null = not yet run / cleared).
  const [testMsg, setTestMsg] = useState<{ text: string; ok: boolean } | null>(
    null,
  );
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

  // Data
  const [deleteMsg, setDeleteMsg] = useState<{ text: string; isError: boolean } | null>(null);
  // Fix #7: inline confirm state replaces window.confirm, which is unreliable /
  // blocked in the Tauri (WRY) webview and matches the pattern already used in
  // HistoryView's "Clear all".
  const [deleteConfirm, setDeleteConfirm] = useState(false);

  // Global state
  const [loadState, setLoadState] = useState<LoadState>("loading");
  // Degraded reason from the daemon's `status` probe — the ONLY method that
  // reports degraded state. Non-null ⇒ daemon is up but its DB is unavailable.
  // (The empty string means "degraded but no machine-readable reason given".)
  const [degradedReason, setDegradedReason] = useState<string | null>(null);
  // Bumped by the Retry / Restart buttons to re-run the load effect.
  const [reloadKey, setReloadKey] = useState(0);
  // Non-null when an OLD daemon survived an upgrade and is still serving old
  // code: holds the stale daemon's build (or "unknown" if it reported none).
  const [staleDaemon, setStaleDaemon] = useState<string | null>(null);
  // The running daemon's reported build_version (for display in the Daemon
  // section); null when offline or not yet known.
  const [daemonVersion, setDaemonVersion] = useState<string | null>(null);

  // Save-config error (separate from the success "Saved")
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
      // Load the popup shortcut independently — it's a Tauri-direct command,
      // not a daemon IPC call, so it works even when the daemon is offline.
      getPopupShortcut()
        .then((s) => {
          if (cancelled) return;
          setCurrentShortcut(s);
          setPendingShortcut(s);
        })
        .catch(() => {
          // Keep default if the Tauri command fails (shouldn't happen in normal operation).
        });

      try {
        // Each call is individually fault-tolerant: a single rejecting method
        // must not blank the screen. The outer try/catch is a backstop only.
        //
        // api.status() is fetched ONCE here and reused for:
        //   (a) the degraded/ready probe (via probeStatus logic inline)
        //   (b) build_version display in the Daemon section
        //   (c) stale-daemon detection (detectStaleDaemonFromStatus)
        // This replaces the previous 3× api.status() pattern (load effect +
        // stale-check effect + detectStaleDaemon's own fetch).
        const [pmResult, cfg, syncSt, daemonSt, myAppVer] = await Promise.all([
          api.getPrivateMode().catch(() => null),
          api.getConfig().catch(() => null),
          api.getSyncStatus().catch(() => null),
          api.status().catch(() => null) as Promise<DaemonStatus | null>,
          appVersion().catch(() => null),
        ]);
        if (cancelled) return;

        // Derive the status probe result from the single status fetch.
        const probe = daemonSt
          ? (daemonSt.degraded === true || daemonSt.ready === false
              ? { kind: "degraded" as const, reason: daemonSt.degraded_reason ?? null }
              : { kind: "ok" as const })
          : { kind: "offline" as const };

        // Record degraded state up front so the banner is correct regardless of
        // which branch we take below (a degraded daemon's DB-gated calls fail,
        // which would otherwise look identical to fully offline).
        setDegradedReason(
          probe.kind === "degraded" ? (probe.reason ?? "") : null,
        );

        // Stale-daemon detection reusing the already-fetched status (no extra round-trip).
        setDaemonVersion(daemonSt?.build_version ?? null);
        if (myAppVer !== null) {
          setStaleDaemon(detectStaleDaemonFromStatus(daemonSt, myAppVer));
        }

        // If the daemon is unreachable for the core calls, show the offline
        // state — UNLESS the status probe says the daemon is actually up but
        // degraded, in which case the dedicated degraded banner is shown instead.
        // (get_sync_status is optional and may be absent on older builds.)
        if (pmResult === null && cfg === null) {
          setLoadState(probe.kind === "degraded" ? "degraded" : "offline");
          setSyncStatus(syncSt);
          return;
        }

        // Guard every field with a safe default — never assume the daemon
        // returned a well-formed, fully-populated object.
        setPrivateMode(pmResult?.private_mode ?? false);
        setConfig({
          p2p_enabled: cfg?.p2p_enabled ?? false,
          supabase_url: cfg?.supabase_url ?? null,
          supabase_anon_key: cfg?.supabase_anon_key ?? null,
        });
        // Prefill Supabase URL: prefer the stored config value, but fall back to
        // the value reported by get_sync_status (may be set via env variable).
        const urlFromStatus = syncSt?.supabase_url ?? null;
        setSupabaseUrl(cfg?.supabase_url ?? urlFromStatus ?? "");
        setSupabaseKey(cfg?.supabase_anon_key ?? "");
        setSyncStatus(syncSt);
        setLoadState("ready");
      } catch (err) {
        if (cancelled) return;
        // Any unexpected error (including a non-IpcError throw): degrade to the
        // offline state rather than letting it propagate and blank the window.
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
  // Degraded = daemon up but its database is unavailable. Driven by the `status`
  // probe in load() (the only method that reports it). The previous derivation
  // keyed off syncStatus.keychain_locked / db_unavailable, which get_sync_status
  // NEVER emits — dead code that meant a degraded daemon was silently mislabeled
  // "offline".
  const degraded = loadState === "degraded";

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
        // Revert on failure and show error
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
  // Sync — Save config
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

  // Run the daemon-side end-to-end Supabase probe and surface a precise,
  // actionable diagnostic instead of leaving the user to guess why sync is
  // silent. Saves the current credentials first so the test reflects the form.
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
      setPendingShortcut(currentShortcut); // revert capture display
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
      // Refresh sync status after setting
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
  // Render
  // -------------------------------------------------------------------------

  const inputCls = [
    "w-64 rounded-ide border border-ide-border bg-ide-bg px-2.5 py-1.5 text-[13px] text-ide-text",
    "outline-none focus:border-ide-accent placeholder:text-ide-faint",
    "disabled:cursor-not-allowed disabled:opacity-40",
  ].join(" ");

  return (
    <ViewShell title="Settings">
      {/* Stale-daemon banner — an OLD daemon survived an upgrade and is still
          serving old code. Offer a one-click restart to the fresh binary. */}
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

      {/* Degraded banner — daemon is UP but its database is unavailable (e.g. the
          SQLCipher key no longer matches). Driven by the `status` probe; the old
          syncStatus.keychain_locked/db_unavailable fields were never emitted. A
          restart can additionally recover a daemon wedged on a transient error. */}
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
          {/* General */}
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

          {/* Daemon — version + manual restart (forces the running daemon to
              the freshly-installed binary after an upgrade). */}
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

          {/* Display */}
          <SectionHeader label="Display" />
          <Panel>
            <SettingsRow label="Preview lines">
              <div className="flex items-center gap-2">
                <input
                  type="range"
                  min={1}
                  max={6}
                  step={1}
                  value={prefs.previewLines}
                  onChange={(e) => setPrefs({ previewLines: Number(e.target.value) })}
                  className="w-28 accent-ide-accent"
                />
                <span className="w-4 text-center text-[13px] text-ide-text">{prefs.previewLines}</span>
              </div>
            </SettingsRow>
            <SettingsRow label="Row height">
              <div className="flex items-center gap-2">
                <input
                  type="range"
                  min={24}
                  max={64}
                  step={4}
                  value={prefs.previewSize}
                  onChange={(e) => setPrefs({ previewSize: Number(e.target.value) })}
                  className="w-28 accent-ide-accent"
                />
                <span className="w-8 text-center text-[13px] text-ide-text">{prefs.previewSize}px</span>
              </div>
            </SettingsRow>
            {/* Maccy parity: image thumbnail height cap */}
            <SettingsRow label="Image thumbnail height">
              <div className="flex flex-col items-end gap-0.5">
                <div className="flex items-center gap-2">
                  <input
                    type="range"
                    min={1}
                    max={200}
                    step={1}
                    value={prefs.imageMaxHeight}
                    onChange={(e) => setPrefs({ imageMaxHeight: Number(e.target.value) })}
                    className="w-28 accent-ide-accent"
                  />
                  <span className="w-10 text-center text-[13px] text-ide-text">
                    {prefs.imageMaxHeight}px
                  </span>
                </div>
                <span className="text-[11px] text-ide-faint">
                  Max height of image previews (1–200 px). Width is always ≤ 340 px.
                </span>
              </div>
            </SettingsRow>
            {/* Maccy parity: history size cap */}
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
                    className={[
                      "w-20 rounded-ide border border-ide-border bg-ide-bg px-2 py-1",
                      "text-[13px] text-ide-text outline-none focus:border-ide-accent",
                    ].join(" ")}
                  />
                  <span className="text-[13px] text-ide-dim">items</span>
                </div>
                <span className="text-[11px] text-ide-faint">
                  Maximum clipboard items shown (1–999).
                </span>
              </div>
            </SettingsRow>
            {/* Maccy parity: hover-preview delay */}
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
                    className={[
                      "w-24 rounded-ide border border-ide-border bg-ide-bg px-2 py-1",
                      "text-[13px] text-ide-text outline-none focus:border-ide-accent",
                    ].join(" ")}
                  />
                  <span className="text-[13px] text-ide-dim">ms</span>
                </div>
                <span className="text-[11px] text-ide-faint">
                  Hover delay before large preview appears (200–100 000 ms).
                  {/* TODO: wire to hover-preview panel when implemented */}
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

          {/* Shortcuts */}
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
                <span className="text-[11px] text-ide-faint">
                  Click the field and press a key combo. OS-reserved combos (e.g.
                  Cmd+Space, Cmd+Tab) cannot be overridden.
                </span>
              </div>
            </SettingsRow>
          </Panel>

          {/* Sync */}
          <SectionHeader label="Sync" />
          {/* Connection status banner — shown when Supabase is configured */}
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
          {/* Account identity note — only when signed in with a known email.
              Supabase RLS isolates rows by auth.uid(): two devices on different
              accounts silently see zero shared rows. Surface the account here so
              the user can spot a mismatch themselves. */}
          {syncStatus !== null &&
            syncStatus.supabase_configured &&
            syncStatus.signed_in &&
            syncStatus.email && (
              <div className="mb-1 rounded-ide border border-ide-border bg-ide-elevated px-3 py-2 text-[12px] text-ide-dim">
                <span className="font-medium text-ide-text">
                  Signed in as {syncStatus.email}
                </span>
                <span className="ml-1">
                  — All your devices must use this same account to sync.
                  Different accounts cannot see each other&apos;s clips.
                </span>
              </div>
            )}
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

          {/* Cloud Sync */}
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
                  Enter the same passphrase on every device to sync.
                </span>
              </div>
            </SettingsRow>
            {/* Sync status block */}
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
                  <StatusRow
                    label="Passphrase set"
                    ok={syncStatus.passphrase_set}
                  />
                  <StatusRow
                    label="Supabase configured"
                    ok={syncStatus.supabase_configured}
                  />
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

          {/* Data */}
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
