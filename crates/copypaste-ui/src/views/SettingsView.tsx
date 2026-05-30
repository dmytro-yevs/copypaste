import { useCallback, useEffect, useRef, useState } from "react";
import { ViewShell } from "../components/ViewShell";
import {
  api,
  IpcError,
  getPopupShortcut,
  setPopupShortcut,
  type AppSettings,
  type SyncStatus,
} from "../lib/ipc";
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

function SubsectionHeader({ label, hint }: { label: string; hint?: string }) {
  return (
    <div className="mb-2 mt-6 first:mt-0">
      <div className="text-[11px] uppercase tracking-wide text-ide-faint">{label}</div>
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
    <div className="flex min-h-[34px] items-center justify-between border-b border-ide-divider px-3 py-1.5 last:border-b-0">
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
  formatValue: (v: number) => string;
}) {
  return (
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
      <span className="w-[52px] text-right text-[13px] text-ide-text">
        {formatValue(value)}
      </span>
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
// ShortcutCapture — focus to record a new key combo
// ---------------------------------------------------------------------------

function eventToAccelerator(e: React.KeyboardEvent<HTMLInputElement>): string | null {
  if (["Meta", "Control", "Alt", "Shift"].includes(e.key)) return null;

  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push("CmdOrCtrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");

  let key: string;
  if (e.code.startsWith("Key")) {
    key = e.code.slice(3);
  } else if (e.code.startsWith("Digit")) {
    key = e.code.slice(5);
  } else {
    key = e.code || e.key;
  }

  if (key.length === 1) {
    key = key.toUpperCase();
  } else {
    const keyMap: Record<string, string> = {
      ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right",
      " ": "Space", Space: "Space", Escape: "Escape", Enter: "Return",
      Return: "Return", Backspace: "Backspace", Delete: "Delete", Tab: "Tab",
      Home: "Home", End: "End", PageUp: "PageUp", PageDown: "PageDown",
      F1: "F1", F2: "F2", F3: "F3", F4: "F4", F5: "F5", F6: "F6",
      F7: "F7", F8: "F8", F9: "F9", F10: "F10", F11: "F11", F12: "F12",
    };
    key = keyMap[key] ?? key;
  }
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

type LoadState = "loading" | "ready" | "offline";

export function SettingsView() {
  const prefs = useUI((s) => s.prefs);
  const setPrefs = useUI((s) => s.setPrefs);

  const [activeTab, setActiveTab] = useState<TabId>("general");

  // General
  const [privateMode, setPrivateMode] = useState(false);
  const [privateModeError, setPrivateModeError] = useState<string | null>(null);
  const pmErrTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Sync
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
  const [saveError, setSaveError] = useState<string | null>(null);
  const saveErrTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const savedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Sync parity — P2P wifi-only toggle
  const [syncOnWifiOnly, setSyncOnWifiOnly] = useState(false);

  // Cloud sync passphrase
  const [passphrase, setPassphrase] = useState("");
  const [passphraseSavedMsg, setPassphraseSavedMsg] = useState<string | null>(null);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const passphraseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Shortcuts
  const [currentShortcut, setCurrentShortcut] = useState("CmdOrCtrl+Shift+V");
  const [pendingShortcut, setPendingShortcut] = useState("CmdOrCtrl+Shift+V");
  const [shortcutMsg, setShortcutMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const shortcutTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Data
  const [deleteMsg, setDeleteMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Global load state
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [reloadKey, setReloadKey] = useState(0);

  // -------------------------------------------------------------------------
  // Load
  // -------------------------------------------------------------------------

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setLoadState("loading");
      getPopupShortcut()
        .then((s) => {
          if (cancelled) return;
          setCurrentShortcut(s);
          setPendingShortcut(s);
        })
        .catch(() => {
          // Keep default if Tauri command fails.
        });

      try {
        const [pmResult, cfg, status] = await Promise.all([
          api.getPrivateMode().catch(() => null),
          api.getConfig().catch(() => null),
          api.getSyncStatus().catch(() => null),
        ]);
        if (cancelled) return;

        if (pmResult === null && cfg === null) {
          setLoadState("offline");
          setSyncStatus(status);
          return;
        }

        setPrivateMode(pmResult?.private_mode ?? false);
        const rawCfg = cfg ?? ({} as Partial<AppSettings>);
        setConfig({
          p2p_enabled: rawCfg.p2p_enabled ?? false,
          supabase_url: rawCfg.supabase_url ?? null,
          supabase_anon_key: rawCfg.supabase_anon_key ?? null,
        });
        const urlFromStatus = status?.supabase_url ?? null;
        setSupabaseUrl(rawCfg.supabase_url ?? urlFromStatus ?? "");
        setSupabaseKey(rawCfg.supabase_anon_key ?? "");
        setSyncStatus(status);
        // Sync parity fields — guarded since AppSettings may not carry them yet.
        setSyncOnWifiOnly((rawCfg as Record<string, unknown>).sync_on_wifi_only === true);
        setLoadState("ready");
      } catch (err) {
        if (cancelled) return;
        void err;
        setLoadState("offline");
      }
    }

    void load();
    return () => { cancelled = true; };
  }, [reloadKey]);

  const offline = loadState !== "ready";

  // -------------------------------------------------------------------------
  // Handlers
  // -------------------------------------------------------------------------

  const handlePrivateMode = useCallback(async (val: boolean) => {
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
  }, [pmErrTimer]);

  const handleSaveConfig = useCallback(async () => {
    // Preserve existing anon key if user left the field blank.
    const trimmedKey = supabaseKey.trim();
    const anonKey: string | null =
      trimmedKey !== "" ? trimmedKey : config.supabase_anon_key;

    const next: AppSettings = {
      p2p_enabled: config.p2p_enabled,
      supabase_url: supabaseUrl.trim() || null,
      supabase_anon_key: anonKey,
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
  }, [config.p2p_enabled, config.supabase_anon_key, supabaseUrl, supabaseKey, saveErrTimer]);

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

  const handleP2pToggle = useCallback(async (val: boolean) => {
    const prev = config.p2p_enabled;
    setConfig((c) => ({ ...c, p2p_enabled: val }));
    try {
      await api.setConfig({ ...config, p2p_enabled: val });
    } catch (err) {
      setConfig((c) => ({ ...c, p2p_enabled: prev }));
      void err;
    }
  }, [config]);

  const handleWifiOnlyToggle = useCallback(async (val: boolean) => {
    setSyncOnWifiOnly(val);
    try {
      // sync_on_wifi_only is a daemon config field; send full config patch.
      await api.setConfig({
        ...config,
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        sync_on_wifi_only: val,
      } as any);
    } catch (err) {
      setSyncOnWifiOnly(!val);
      void err;
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [config, syncOnWifiOnly]);

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
  // Shared CSS classes
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

  const btnCls = [
    "rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
    "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
  ].join(" ");

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
              onChange={(v) => setPrefs({ playSoundOnCopy: v })}
            />
          </SettingsRow>
          <SettingsRow label="Show notification on copy">
            <Toggle
              checked={prefs.notifyOnCopy}
              onChange={(v) => setPrefs({ notifyOnCopy: v })}
            />
          </SettingsRow>
          <SettingsRow label="Mask sensitive data">
            <Toggle
              checked={prefs.maskSensitive}
              onChange={(v) => setPrefs({ maskSensitive: v })}
            />
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
          <SettingsRow label="Items displayed">
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
              <span className="text-[11px] text-ide-faint">1–999</span>
            </div>
          </SettingsRow>
          <SettingsRow label="Preview hover delay">
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
              <span className="text-[11px] text-ide-faint">200–100 000 ms</span>
            </div>
          </SettingsRow>
        </Panel>

        <SubsectionHeader label="Popup appearance" hint="How the popup looks when triggered." />
        <Panel>
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
          <SettingsRow label="Image preview height">
            <div className="flex flex-col items-end gap-0.5">
              <SliderRow
                min={1}
                max={200}
                step={1}
                value={prefs.imageMaxHeight}
                onChange={(v) => setPrefs({ imageMaxHeight: v })}
                formatValue={(v) => `${v}px`}
              />
              <span className="text-[11px] text-ide-faint">Max image thumbnail height (1–200 px)</span>
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
            <Toggle
              checked={config.p2p_enabled}
              onChange={(v) => void handleP2pToggle(v)}
              disabled={offline}
            />
          </SettingsRow>
          <SettingsRow label="Sync on Wi-Fi only">
            <Toggle
              checked={syncOnWifiOnly}
              onChange={(v) => void handleWifiOnlyToggle(v)}
              disabled={offline}
            />
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
                  className={btnCls}
                >
                  Set
                </button>
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
              <span className="text-[11px] text-ide-faint">
                Same passphrase on every device to sync.
              </span>
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
                <ShortcutCapture
                  value={pendingShortcut}
                  onChange={setPendingShortcut}
                />
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
              <span className="text-[11px] text-ide-faint">
                Click then press a combo. OS-reserved keys (Cmd+Space etc.) cannot be overridden.
              </span>
            </div>
          </SettingsRow>
        </Panel>
      </div>
    );
  }

  function renderStorage() {
    return (
      <div className="space-y-2">
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
      {/* Offline banner */}
      {loadState === "offline" && (
        <div className="mb-4 flex items-center justify-between gap-3 rounded-ide border border-ide-border bg-ide-elevated px-3 py-2 text-[13px] text-ide-dim">
          <span>Daemon not running — clipboard sync paused.</span>
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
      )}

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
