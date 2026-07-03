// useSettingsState.ts — extracted from SettingsView.tsx (CopyPaste-g06m.35)
// Contains all state, refs, effects, and handlers for the Settings screen.
// SettingsView.tsx is now a thin composition root that calls this hook.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { type LoadState } from "../../../lib/loadState";
import { emit, listen } from "@tauri-apps/api/event";
import {
  api,
  ipcErrorMessage,
  isIpcNotReady,
  IpcError,
  probeStatus,
  appVersion,
  getPopupShortcut,
  getDefaultPopupShortcut,
  setPopupShortcut,
  restartDaemon,
  detectStaleDaemonFromStatus,
  getAllowScreenshots,
  setAllowScreenshots,
  type AppSettings,
  type SyncStatus,
  type DaemonStatus,
  type PairedDevice,
} from "../../../lib/ipc";
import { useUI } from "../../../store";
import { isNotificationPermissionGranted } from "../../../lib/notificationPermission";
import {
  snapToNearest,
  DEFAULT_POPUP_SHORTCUT,
  TEXT_SIZE_STEPS_BYTES,
  IMAGE_SIZE_STEPS_BYTES,
  FILE_SIZE_STEPS_BYTES,
  QUOTA_STEPS_BYTES,
  SENSITIVE_TTL_STEPS,
  DEFAULT_MAX_TEXT_BYTES,
  DEFAULT_MAX_IMAGE_BYTES,
  DEFAULT_MAX_FILE_BYTES,
  DEFAULT_STORAGE_QUOTA_BYTES,
  DEFAULT_SENSITIVE_TTL_SECS,
} from "../lib/settingsSliders";

// #14: LoadState is now defined in the shared lib/loadState.ts module (superset).
// Re-exported for backward compat with consumers that import it from this path.
export type { LoadState } from "../../../lib/loadState";

// v0.5.3: inputs use global base styles from index.css; only width/padding overrides needed here
export const INPUT_CLS = [
  "w-64 px-2.5 py-1.5 text-[13px]",
  "disabled:cursor-not-allowed disabled:opacity-40",
].join(" ");

// borderRadius is applied via inline style (var(--r-ctl)) on every button using btnCls.
// Do NOT add rounded-ide here — use btnStyle instead.
export const BTN_CLS = [
  "border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
  "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
].join(" ");

export const BTN_STYLE = { borderRadius: "var(--r-ctl)" } as const;

export function useSettingsState() {
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
    p2p_enabled: true,
    supabase_url: null,
    supabase_anon_key: null,
  });
  const [supabaseUrl, setSupabaseUrl] = useState("");
  const [supabaseKey, setSupabaseKey] = useState("");
  // jhvl: Supabase GoTrue email + password for email+password sign-in.
  // These are write-only fields — the daemon never returns them, so the UI
  // can only show a presence flag (supabase_email_set / supabase_password_set).
  // The inputs are always cleared after a successful Save to avoid holding
  // credentials in memory longer than necessary.
  const [supabaseEmail, setSupabaseEmail] = useState("");
  const [supabasePassword, setSupabasePassword] = useState("");
  const [relayUrl, setRelayUrl] = useState("");
  const [savedMsg, setSavedMsg] = useState(false);
  const [testMsg, setTestMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const [testing, setTesting] = useState(false);

  // Cloud sync passphrase
  const [passphrase, setPassphrase] = useState("");
  const [passphraseSavedMsg, setPassphraseSavedMsg] = useState<string | null>(null);
  // CopyPaste-crh3.51: explicit success flag for the passphrase feedback colour,
  // so SyncTab no longer infers success from a fragile `=== "Saved"` text match
  // (any wording change would silently turn a success message red).
  const [passphraseSaveOk, setPassphraseSaveOk] = useState(false);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  // CopyPaste-yw2k: paired peers list, loaded alongside the other settings so
  // cloudAccountMismatch can be derived by comparing each peer's supabase_account_id
  // against the local one. Null until the first successful load.
  const [pairedPeers, setPairedPeers] = useState<PairedDevice[] | null>(null);

  // Shortcuts
  const [currentShortcut, setCurrentShortcut] = useState(DEFAULT_POPUP_SHORTCUT);
  const [pendingShortcut, setPendingShortcut] = useState(DEFAULT_POPUP_SHORTCUT);
  // CopyPaste-sqw0: fetched from Rust via `get_default_popup_shortcut` at load
  // time so the "reset to default" button always reflects the Rust constant,
  // not the TS fallback literal.  Starts as DEFAULT_POPUP_SHORTCUT while the
  // IPC call is in-flight; updated by the useEffect below.
  const [defaultShortcut, setDefaultShortcut] = useState(DEFAULT_POPUP_SHORTCUT);
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
  // §6.3: History display limit — read from and written to the persisted UIPrefs store.
  // maxItems computed inside StorageTab (CopyPaste-g06m.14 split).
  // Per-field save feedback: key = field name, value typed {ok, message} | null.
  // bdac.106: typed signal replaces brittle msg!=="Saved" string comparison.
  const [limitsMsg, setLimitsMsg] = useState<Record<string, { ok: boolean; message: string } | null>>({});
  const limitsMsgTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({});

  // j9xj (PG-30): master sync kill-switch — Android parity. True = sync enabled
  // (default). False = all transports disabled. Daemon implements sync_enabled
  // in AppConfig (tke7/PG-30); the toggle has full effect.
  const [syncEnabled, setSyncEnabled] = useState(true);
  // 7set: true when the daemon's get_config response did NOT include sync_enabled.
  // In that case the toggle has no runtime effect (daemon ignores it) and we show
  // a warning so the user knows. Reset to false once the daemon sends the field.
  const [syncEnabledStub, setSyncEnabledStub] = useState(false);

  // Sync parity — p2p toggle + wifi-only
  const [syncOnWifiOnly, setSyncOnWifiOnly] = useState(false);

  // LAN visibility — mDNS-SD advertisement toggle (config.toml, hot-applied).
  const [lanVisibility, setLanVisibility] = useState(true);

  // auto_apply_synced_clip — writes incoming synced items to the local clipboard.
  // Daemon default is true; mirror that here so new installs start in sync.
  const [autoApplySyncedClip, setAutoApplySyncedClip] = useState(true);

  // Capture — daemon AppConfig fields (config.toml).
  // am9w: daemon defaults collect_public_ip to false (opt-out); mirror that here.
  const [collectPublicIp, setCollectPublicIp] = useState(false);
  const [pasteAsPlainText, setPasteAsPlainText] = useState(false);
  const [excludedApps, setExcludedApps] = useState<string[]>([]);
  // Text buffer for the "add excluded app" input.
  const [newExcludedApp, setNewExcludedApp] = useState("");
  // CopyPaste-6uy9: allow-screenshots preference — Tauri-direct (not daemon).
  // false = content protection ON (PG-25 default); true = screenshots allowed.
  const [allowScreenshots, setAllowScreenshots_state] = useState(false);
  const [allowScreenshotsError, setAllowScreenshotsError] = useState<string | null>(null);
  const allowScreenshotsErrTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Sync-path restart guard: true while restart_daemon is in flight after a
  // sync-path toggle (P2P/relay/Supabase). Disables the control so rapid
  // double-toggles can't queue two restarts.
  const [syncRestarting, setSyncRestarting] = useState(false);
  const syncRestartTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Data
  const [deleteMsg, setDeleteMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState(false);

  // gq51: Vacuum + stats state
  const [vacuumBusy, setVacuumBusy] = useState(false);
  const [vacuumMsg, setVacuumMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const [dbStats, setDbStats] = useState<{ item_count: number; size_bytes: number } | null>(null);
  const vacuumMsgTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 85n9: Backup / Restore state
  const [exportInProgress, setExportInProgress] = useState(false);
  const [exportMsg, setExportMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const [exportIncludeSensitive, setExportIncludeSensitive] = useState(false);
  const [importInProgress, setImportInProgress] = useState(false);
  const [importMsg, setImportMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const exportMsgTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const importMsgTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // vcnv: pending parsed backup items held until the user confirms the restore modal.
  const [importPending, setImportPending] = useState<unknown[] | null>(null);

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

  // CopyPaste-vrur / CopyPaste-1jms.29: notification permission denial warning.
  // Check on mount and whenever notifyOnCopy changes.
  // Uses isNotificationPermissionGranted() which calls the Tauri
  // check_notification_permission command (macOS UNUserNotificationCenter).
  // Notification.permission (browser WKWebView API) is NOT used — it does NOT
  // reflect macOS system notification state in a Tauri WKWebView.
  const [notifPermDenied, setNotifPermDenied] = useState(false);
  useEffect(() => {
    isNotificationPermissionGranted().then((granted) => {
      setNotifPermDenied(!granted);
    });
  }, [prefs.notifyOnCopy]);

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
      if (exportMsgTimerRef.current !== null) clearTimeout(exportMsgTimerRef.current);
      if (importMsgTimerRef.current !== null) clearTimeout(importMsgTimerRef.current);
      if (vacuumMsgTimerRef.current !== null) clearTimeout(vacuumMsgTimerRef.current);
      if (allowScreenshotsErrTimer.current !== null) clearTimeout(allowScreenshotsErrTimer.current);
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

      // CopyPaste-6uy9: allow-screenshots is Tauri-direct (not daemon-backed).
      getAllowScreenshots()
        .then((v) => {
          if (cancelled) return;
          setAllowScreenshots_state(v);
        })
        .catch(() => {
          // Non-fatal: keep the default false (protection ON).
        });

      // CopyPaste-sqw0: fetch the authoritative default shortcut from Rust so
      // the "reset to default" button always reflects the Rust constant, never
      // a stale TS literal.  Falls back to DEFAULT_POPUP_SHORTCUT on failure.
      getDefaultPopupShortcut()
        .then((d) => {
          if (cancelled) return;
          setDefaultShortcut(d);
        })
        .catch(() => {
          // Non-fatal: defaultShortcut stays at the TS fallback literal.
        });

      try {
        // api.status() is fetched once and reused for: degraded probe, build_version
        // display, and stale-daemon detection — avoids three separate round-trips.
        // CopyPaste-yw2k: listPeers is included here so cloudAccountMismatch can
        // be derived synchronously in the return object (no extra render cycle).
        const [pmResult, cfg, syncSt, daemonSt, myAppVer, peersResult] = await Promise.all([
          api.getPrivateMode().catch(() => null),
          api.getConfig().catch(() => null),
          api.getSyncStatus().catch(() => null),
          api.status().catch(() => null) as Promise<DaemonStatus | null>,
          appVersion().catch(() => null),
          api.listPeers().catch(() => null),
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
          // tk2j: probe.kind tells us WHY the calls failed. Only show "offline"
          // when the daemon is actually unreachable (status also failed → kind
          // "offline"). When the daemon answered status but cfg/pm still failed
          // (kind "ok"), surface a generic error so the user is not misled.
          if (probe.kind === "degraded") {
            setLoadState("degraded");
          } else if (probe.kind === "ok") {
            setLoadState("error");
          } else {
            setLoadState("offline");
          }
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
        // CopyPaste-yw2k: store the peer list so cloudAccountMismatch can be
        // computed. Non-fatal: null when the daemon rejects list_peers (e.g.
        // not-ready) — mismatch stays false until peers are known.
        setPairedPeers(peersResult?.peers ?? null);

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

        // Sync parity
        setSyncOnWifiOnly(rawCfg.sync_on_wifi_only ?? false);
        // j9xj (PG-30): hydrate master sync_enabled (daemon may not emit it yet;
        // absent/null → true so existing installs stay in "sync on" state).
        // 7set: track whether the daemon supports this field so we can warn when absent.
        const syncEnabledSupported = rawCfg.sync_enabled !== undefined && rawCfg.sync_enabled !== null;
        setSyncEnabledStub(!syncEnabledSupported);
        setSyncEnabled(rawCfg.sync_enabled ?? true);

        // Capture — these AppConfig fields are not in the AppSettings
        // interface (kept in lib/ipc.ts), so read them off the raw response with
        // a narrow typed view rather than `any`.
        const privacyCfg = rawCfg as {
          collect_public_ip?: boolean | null;
          paste_as_plain_text?: boolean | null;
          excluded_app_bundle_ids?: string[] | null;
          lan_visibility?: boolean | null;
        };
        // am9w: absent value → opt-out (false), consistent with daemon #[serde(default)].
        setCollectPublicIp(privacyCfg.collect_public_ip ?? false);
        setPasteAsPlainText(privacyCfg.paste_as_plain_text ?? false);
        setExcludedApps(privacyCfg.excluded_app_bundle_ids ?? []);
        // lan_visibility defaults to true (LAN-visible) on first install.
        setLanVisibility(privacyCfg.lan_visibility ?? true);
        // auto_apply_synced_clip defaults to true (daemon default) on first install.
        setAutoApplySyncedClip(rawCfg.auto_apply_synced_clip ?? true);

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

        // gq51: fetch db stats best-effort after the main load succeeds.
        // Failure is non-fatal — stats simply won't display (older daemons that
        // don't support the db_stats verb will reject with an IpcError).
        api.getDbStats().then((stats) => {
          if (!cancelled) setDbStats(stats);
        }).catch(() => {
          // Non-fatal: db_stats not supported on this daemon version.
        });
      } catch (err) {
        if (cancelled) return;
        // tk2j: mirror DevicesView — only mark offline when the transport error
        // explicitly says "daemon_offline". Other IpcErrors mean the daemon IS up
        // but the call failed (e.g. DB error) — probe status to distinguish.
        if (err instanceof IpcError && err.code === "daemon_offline") {
          setLoadState("offline");
          return;
        }
        if (isIpcNotReady(err)) {
          setLoadState("not_ready");
          return;
        }
        // Daemon answered but the IPC call failed — probe status to tell a
        // degraded daemon apart from a generic error. This avoids mislabeling
        // a DB-degraded daemon as "not running".
        const probe = await probeStatus();
        if (probe.kind === "offline") {
          setLoadState("offline");
        } else if (probe.kind === "degraded") {
          setLoadState("degraded");
        } else {
          setLoadState("error");
        }
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
    //
    // audit P1-7: outside the Tauri runtime (plain browser / ?mock=1) the event
    // plugin is absent, so listen() rejected and logged a console error on every
    // mount. Feature-detect the runtime (same gate HistoryView uses) and skip the
    // subscription in the browser — there's no tray to emit the event anyway.
    const hasTauri =
      typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    const unlistenPromise = hasTauri
      ? listen<boolean>("private-mode-changed", (event) => {
          if (!cancelled && typeof event.payload === "boolean") {
            setPrivateMode(event.payload);
          }
        })
      : null;

    return () => {
      cancelled = true;
      window.removeEventListener("focus", resyncPrivateMode);
      document.removeEventListener("visibilitychange", onVisibility);
      void unlistenPromise?.then((unlisten) => unlisten());
    };
  }, []);

  // -------------------------------------------------------------------------
  // Helpers — per-field limits save with feedback
  // -------------------------------------------------------------------------

  // bdac.106: ok=true → success colour; ok=false → error colour.
  // Callers pass ok explicitly — no more string !== "Saved" comparison.
  function showLimitsMsg(field: string, msg: string | null, durationMs: number, ok = false) {
    if (limitsMsgTimers.current[field] !== undefined) {
      clearTimeout(limitsMsgTimers.current[field]);
    }
    setLimitsMsg((prev) => ({ ...prev, [field]: msg !== null ? { ok, message: msg } : null }));
    if (msg !== null) {
      limitsMsgTimers.current[field] = setTimeout(
        () => setLimitsMsg((prev) => ({ ...prev, [field]: null })),
        durationMs
      );
    }
  }

  // Build the full AppSettings patch for set_config, merging current config
  // with any updated limits fields. Slider values are already raw bytes/counts/secs.
  function buildConfigPatch(overrides: Partial<AppSettings>): AppSettings {
    return {
      // j9xj (PG-30): include master sync_enabled in every patch so it is
      // preserved across other config saves. Daemon ignores unknown fields.
      sync_enabled: syncEnabled,
      p2p_enabled: config.p2p_enabled,
      supabase_url: supabaseUrl.trim() || null,
      supabase_anon_key: supabaseKey.trim() || null,
      relay_url: relayUrl.trim() || null,
      max_text_size_bytes: maxTextBytes,
      max_image_size_bytes: maxImageBytes,
      max_file_size_bytes: maxFileBytes,
      storage_quota_bytes: quotaBytes,
      sensitive_ttl_secs: sensitiveTtlSecs,
      sync_on_wifi_only: syncOnWifiOnly,
      sound_on_copy: prefs.playSoundOnCopy,
      notify_on_copy: prefs.notifyOnCopy,
      collect_public_ip: collectPublicIp,
      paste_as_plain_text: pasteAsPlainText,
      excluded_app_bundle_ids: excludedApps,
      lan_visibility: lanVisibility,
      auto_apply_synced_clip: autoApplySyncedClip,
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
    // CopyPaste-8ebg.20 fix: bail out BEFORE the optimistic setState when
    // config isn't loaded yet. The old order applied the optimistic update
    // unconditionally and only skipped the persist call afterwards, so the
    // edit appeared in the UI, was never saved, and silently vanished on the
    // next reload.
    if (loadState !== "ready") return;
    const next = [...excludedApps, id];
    const prev = excludedApps;
    setExcludedApps(next);
    setNewExcludedApp("");
    try {
      await api.setConfig(
        buildConfigPatch({ excluded_app_bundle_ids: next }),
      );
    } catch {
      setExcludedApps(prev);
    }
  }

  // Remove a bundle ID from the excluded-apps list and persist. Reverts on failure.
  async function removeExcludedApp(bundleId: string) {
    // CopyPaste-8ebg.20 fix: same ordering fix as addExcludedApp — gate the
    // optimistic update on loadState readiness instead of applying it first.
    if (loadState !== "ready") return;
    const next = excludedApps.filter((b) => b !== bundleId);
    const prev = excludedApps;
    setExcludedApps(next);
    try {
      await api.setConfig(
        buildConfigPatch({ excluded_app_bundle_ids: next }),
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
      await api.setConfig(buildConfigPatch(patch));
    } catch (err) {
      const msg = ipcErrorMessage(err, "Save failed");
      showLimitsMsg(field, msg, 4000, false);
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
        const msg = ipcErrorMessage(err, "Failed to update private mode");
        setPrivateModeError(msg);
        if (pmErrTimer.current !== null) clearTimeout(pmErrTimer.current);
        pmErrTimer.current = setTimeout(() => setPrivateModeError(null), 3500);
      }
    },
    []
  );

  // -------------------------------------------------------------------------
  // General — Allow screenshots (CopyPaste-6uy9)
  // -------------------------------------------------------------------------

  const handleAllowScreenshots = useCallback(
    async (val: boolean) => {
      setAllowScreenshots_state(val);
      setAllowScreenshotsError(null);
      try {
        await setAllowScreenshots(val);
      } catch (err) {
        // Revert on failure.
        setAllowScreenshots_state(!val);
        const msg = ipcErrorMessage(err, "Failed to update screenshot protection");
        setAllowScreenshotsError(msg);
        if (allowScreenshotsErrTimer.current !== null) clearTimeout(allowScreenshotsErrTimer.current);
        allowScreenshotsErrTimer.current = setTimeout(() => setAllowScreenshotsError(null), 3500);
      }
    },
    []
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

    // jhvl: Only include email/password when the user has typed a non-empty value.
    // Sending null would clear the stored credential; omitting the field preserves it.
    const trimmedEmail = supabaseEmail.trim();
    // 3c72: trim whitespace so accidental leading/trailing spaces do not cause
    // silent auth failures — mirrors the trimmedEmail handling above.
    const trimmedPassword = supabasePassword.trim();
    const next: AppSettings = {
      p2p_enabled: config.p2p_enabled,
      supabase_url: supabaseUrl.trim() || null,
      supabase_anon_key: anonKey,
      relay_url: relayUrl.trim() || null,
      ...(trimmedEmail ? { supabase_email: trimmedEmail } : {}),
      ...(trimmedPassword ? { supabase_password: trimmedPassword } : {}),
    };
    setSaveError(null);
    try {
      await api.setConfig(next);
      // Clear the credential inputs after a successful save — they were write-only.
      // The presence flags (supabase_email_set / supabase_password_set) will be
      // refreshed on the next getSyncStatus call (triggered by the daemon restart).
      if (trimmedEmail) setSupabaseEmail("");
      if (trimmedPassword) setSupabasePassword("");
      setConfig(next);
      setSavedMsg(true);
      if (savedTimerRef.current !== null) clearTimeout(savedTimerRef.current);
      savedTimerRef.current = setTimeout(() => setSavedMsg(false), 2500);
      // Supabase URL/key are read at daemon startup — restart so the new
      // credentials take effect immediately without requiring a manual relaunch.
      setSyncRestarting(true);
      try {
        await restartDaemon();
      } catch (restartErr) {
        // CopyPaste-8ebg.19 fix: a failed restart is NOT non-fatal — the
        // daemon keeps running with the OLD credentials and sync breaks
        // silently while the UI still shows "Saved". Surface the failure the
        // same way handleP2pToggle does instead of swallowing it.
        setSavedMsg(false);
        if (savedTimerRef.current !== null) clearTimeout(savedTimerRef.current);
        const msg =
          restartErr instanceof Error
            ? restartErr.message
            : "Saved, but restarting the sync service failed — relaunch the app to apply the new credentials.";
        setSaveError(msg);
        if (saveErrTimer.current !== null) clearTimeout(saveErrTimer.current);
        saveErrTimer.current = setTimeout(() => setSaveError(null), 4000);
      } finally {
        setSyncRestarting(false);
      }
      // CopyPaste-crh3.50: signal success so handleTestConnection can abort on a
      // failed save instead of testing against the stale daemon config.
      return true;
    } catch (err) {
      const msg = ipcErrorMessage(err, "Save failed");
      setSaveError(msg);
      if (saveErrTimer.current !== null) clearTimeout(saveErrTimer.current);
      saveErrTimer.current = setTimeout(() => setSaveError(null), 3500);
      return false;
    }
  }, [config.p2p_enabled, config.supabase_anon_key, supabaseUrl, supabaseKey, supabaseEmail, supabasePassword, relayUrl]);

  const handleTestConnection = useCallback(async () => {
    setTesting(true);
    setTestMsg(null);
    try {
      // CopyPaste-crh3.50: do NOT test against the previous config when the save
      // silently failed — handleSaveConfig already surfaced the error.
      const saved = await handleSaveConfig();
      if (!saved) {
        setTestMsg({ text: "Fix the save error above, then test again.", ok: false });
        return;
      }
      const result = await api.testCloudConnection();
      setTestMsg({ text: result.message, ok: result.ok });
    } catch (err) {
      const msg = ipcErrorMessage(err, "Connection test unavailable (daemon offline or cloud-sync not built in)");
      setTestMsg({ text: msg, ok: false });
    } finally {
      setTesting(false);
    }
  }, [handleSaveConfig]);

  // -------------------------------------------------------------------------
  // Sync parity — p2p toggle + wifi-only
  // -------------------------------------------------------------------------

  // j9xj (PG-30): master sync_enabled toggle. NOT memoized for same reason as
  // handleP2pToggle — buildConfigPatch closes over live slider state.
  const handleSyncEnabledToggle = async (val: boolean) => {
    const prev = syncEnabled;
    setSyncEnabled(val);
    await saveLimitsField(
      "sync_enabled",
      { sync_enabled: val },
      () => setSyncEnabled(prev),
    );
  };

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
        buildConfigPatch({ p2p_enabled: val }),
      );
      // The daemon only reads p2p_enabled at startup — restart so the new
      // value takes effect immediately. Show a transient status message and
      // disable the toggle while the restart is in flight to prevent queuing
      // a second restart from a rapid double-click.
      setSyncRestarting(true);
      showLimitsMsg("p2p_enabled", "Restarting sync service…", 6000, true);
      try {
        await restartDaemon();
        showLimitsMsg("p2p_enabled", "Sync service restarted", 2500, true);
      } catch (restartErr) {
        const msg =
          restartErr instanceof Error ? restartErr.message : "Restart failed — relaunch the app";
        showLimitsMsg("p2p_enabled", msg, 4000, false);
      } finally {
        setSyncRestarting(false);
        if (syncRestartTimerRef.current !== null) clearTimeout(syncRestartTimerRef.current);
      }
    } catch (err) {
      // Revert on set_config failure — no restart attempted.
      setConfig((c) => ({ ...c, p2p_enabled: prev }));
      const msg = ipcErrorMessage(err, "Failed to update P2P setting");
      showLimitsMsg("p2p_enabled", msg, 4000, false);
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

  // NOT memoized — same reasoning as handleWifiOnlyToggle above.
  const handleLanVisibilityToggle = async (val: boolean) => {
    const prev = lanVisibility;
    setLanVisibility(val);
    await saveLimitsField(
      "lan_visibility",
      { lan_visibility: val },
      () => setLanVisibility(prev),
    );
  };

  // NOT memoized — same reasoning as handleWifiOnlyToggle above.
  const handleAutoApplySyncedClipToggle = async (val: boolean) => {
    const prev = autoApplySyncedClip;
    setAutoApplySyncedClip(val);
    await saveLimitsField(
      "auto_apply_synced_clip",
      { auto_apply_synced_clip: val },
      () => setAutoApplySyncedClip(prev),
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
  // CopyPaste-sqw0: uses `defaultShortcut` (fetched from Rust via
  // `get_default_popup_shortcut`) rather than the TS literal so the two sides
  // share the same value at runtime.
  const handleResetShortcut = useCallback(async () => {
    if (currentShortcut === defaultShortcut) {
      setPendingShortcut(defaultShortcut);
      return;
    }
    setPendingShortcut(defaultShortcut);
    try {
      await setPopupShortcut(defaultShortcut);
      setCurrentShortcut(defaultShortcut);
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
  }, [currentShortcut, defaultShortcut]);

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
      setPassphraseSaveOk(true);
      if (passphraseTimerRef.current !== null) clearTimeout(passphraseTimerRef.current);
      passphraseTimerRef.current = setTimeout(() => setPassphraseSavedMsg(null), 2500);
    } catch (err) {
      const msg = ipcErrorMessage(err, "Error");
      setPassphraseSavedMsg(msg);
      setPassphraseSaveOk(false);
      if (passphraseTimerRef.current !== null) clearTimeout(passphraseTimerRef.current);
      passphraseTimerRef.current = setTimeout(() => setPassphraseSavedMsg(null), 3000);
    }
  }, [passphrase]);

  // -------------------------------------------------------------------------
  // Data — Delete all
  // -------------------------------------------------------------------------

  const handleDeleteAll = useCallback(async () => {
    // Modal already closed by caller before invoking this.
    try {
      const result = await api.deleteAll();
      setDeleteMsg({
        text: `Deleted ${result.deleted} item${result.deleted === 1 ? "" : "s"}`,
        isError: false,
      });
      if (deleteTimerRef.current !== null) clearTimeout(deleteTimerRef.current);
      deleteTimerRef.current = setTimeout(() => setDeleteMsg(null), 3000);
    } catch (err) {
      const msg = ipcErrorMessage(err, "Clear failed");
      setDeleteMsg({ text: msg, isError: true });
      if (deleteTimerRef.current !== null) clearTimeout(deleteTimerRef.current);
      deleteTimerRef.current = setTimeout(() => setDeleteMsg(null), 4000);
    }
  }, [deleteTimerRef]);

  // -------------------------------------------------------------------------
  // gq51: Vacuum — compact the SQLite WAL and refresh stats afterwards
  // -------------------------------------------------------------------------

  const handleVacuum = useCallback(async () => {
    if (vacuumBusy) return;
    setVacuumBusy(true);
    setVacuumMsg(null);
    try {
      await api.vacuum();
      setVacuumMsg({ text: "Vacuum done — database compacted", isError: false });
      // Refresh stats so the new size is shown immediately.
      api.getDbStats().then((stats) => setDbStats(stats)).catch(() => {});
      if (vacuumMsgTimerRef.current !== null) clearTimeout(vacuumMsgTimerRef.current);
      vacuumMsgTimerRef.current = setTimeout(() => setVacuumMsg(null), 4000);
    } catch (err) {
      const msg = ipcErrorMessage(err, "Vacuum failed");
      setVacuumMsg({ text: msg, isError: true });
      if (vacuumMsgTimerRef.current !== null) clearTimeout(vacuumMsgTimerRef.current);
      vacuumMsgTimerRef.current = setTimeout(() => setVacuumMsg(null), 5000);
    } finally {
      setVacuumBusy(false);
    }
  }, [vacuumBusy]);

  // -------------------------------------------------------------------------
  // 85n9: Backup — export clipboard history as a downloaded JSON file
  // -------------------------------------------------------------------------

  const handleExport = useCallback(async () => {
    if (exportInProgress) return;
    setExportInProgress(true);
    setExportMsg(null);
    try {
      const data = await api.exportItems(exportIncludeSensitive);
      const json = JSON.stringify(data, null, 2);

      // Trigger a browser download via Blob + temporary <a download> anchor.
      // No fs/dialog Tauri plugin is needed — the same pattern is used by
      // FileChip's "Save As" button (triggerDownload in FileChip.tsx).
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `copypaste-backup-${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(anchor);
      anchor.click();
      document.body.removeChild(anchor);
      // Revoke after a short delay so the download starts before the blob is freed.
      setTimeout(() => URL.revokeObjectURL(url), 10_000);

      const count = (data.items ?? []).length;
      setExportMsg({ text: `Exported ${count} item${count === 1 ? "" : "s"}`, isError: false });
      if (exportMsgTimerRef.current !== null) clearTimeout(exportMsgTimerRef.current);
      exportMsgTimerRef.current = setTimeout(() => setExportMsg(null), 4000);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setExportMsg({ text: `Export failed: ${msg}`, isError: true });
      if (exportMsgTimerRef.current !== null) clearTimeout(exportMsgTimerRef.current);
      exportMsgTimerRef.current = setTimeout(() => setExportMsg(null), 5000);
    } finally {
      setExportInProgress(false);
    }
  }, [exportInProgress, exportIncludeSensitive]);

  // -------------------------------------------------------------------------
  // 85n9: Restore — import clipboard history from a JSON backup file
  // -------------------------------------------------------------------------

  const handleImportFile = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      // Reset the input so the same file can be re-selected after an error.
      e.target.value = "";
      if (!file) return;

      setImportMsg(null);
      try {
        // Read the file as text using the browser FileReader API — no fs Tauri
        // plugin capability is needed; FileReader works in Tauri's webview.
        const text = await new Promise<string>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(reader.result as string);
          reader.onerror = () => reject(new Error("Failed to read file"));
          reader.readAsText(file);
        });

        let parsed: { items?: unknown[] };
        try {
          parsed = JSON.parse(text) as { items?: unknown[] };
        } catch {
          throw new Error("Invalid JSON — file may be corrupted or wrong format");
        }

        const items = parsed.items;
        if (!Array.isArray(items)) {
          throw new Error('Invalid backup file — expected { "items": [...] }');
        }
        if (items.length === 0) {
          setImportMsg({ text: "No items in backup file", isError: false });
          if (importMsgTimerRef.current !== null) clearTimeout(importMsgTimerRef.current);
          importMsgTimerRef.current = setTimeout(() => setImportMsg(null), 3000);
          return;
        }

        // vcnv: hold parsed items and show a confirmation modal before touching
        // the live database. The actual import runs in handleConfirmImport().
        setImportPending(items);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setImportMsg({ text: `Import failed: ${msg}`, isError: true });
        if (importMsgTimerRef.current !== null) clearTimeout(importMsgTimerRef.current);
        importMsgTimerRef.current = setTimeout(() => setImportMsg(null), 5000);
      }
    },
    [],
  );

  // vcnv: perform the actual import after the user confirmed the modal.
  const handleConfirmImport = useCallback(async () => {
    const items = importPending;
    setImportPending(null);
    if (!items || items.length === 0) return;

    setImportInProgress(true);
    setImportMsg(null);
    try {
      const result = await api.importItems(items);
      const { inserted, skipped } = result;
      setImportMsg({
        text: `Imported ${inserted} item${inserted === 1 ? "" : "s"}${skipped > 0 ? `, ${skipped} skipped (duplicates)` : ""}`,
        isError: false,
      });
      if (importMsgTimerRef.current !== null) clearTimeout(importMsgTimerRef.current);
      importMsgTimerRef.current = setTimeout(() => setImportMsg(null), 5000);
      // h97m: notify HistoryView (and any other view) to refresh so imported
      // items appear immediately without waiting for the next poll interval.
      // Fire-and-forget; failure just means the other view refreshes later.
      void emit("history-refresh", null).catch(() => {});
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setImportMsg({ text: `Import failed: ${msg}`, isError: true });
      if (importMsgTimerRef.current !== null) clearTimeout(importMsgTimerRef.current);
      importMsgTimerRef.current = setTimeout(() => setImportMsg(null), 5000);
    } finally {
      setImportInProgress(false);
    }
  }, [importPending]);

  // CopyPaste-crh3.49: memoize the cloud account-mismatch detection so the
  // pairedPeers iteration only re-runs when the inputs change, not on every
  // render of this 30+ state-var hook (vacuum timer, passphrase timer, etc.).
  const cloudAccountMismatch = useMemo(() => {
    const localId = syncStatus?.supabase_account_id ?? null;
    // If we don't know our own id, we can't detect a mismatch.
    if (localId == null) return false;
    // If no peers loaded yet, stay false (no false positives).
    if (pairedPeers == null) return false;
    return pairedPeers.some(
      (p) => p.supabase_account_id != null && p.supabase_account_id !== localId,
    );
  }, [syncStatus?.supabase_account_id, pairedPeers]);

  return {
    // Display prefs
    prefs,
    setPrefs,
    // General
    privateMode,
    privateModeError,
    notifPermDenied,
    collectPublicIp,
    setCollectPublicIp,
    pasteAsPlainText,
    setPasteAsPlainText,
    allowScreenshots,
    allowScreenshotsError,
    excludedApps,
    newExcludedApp,
    setNewExcludedApp,
    daemonVersion,
    // Sync
    syncEnabled,
    syncEnabledStub,
    syncOnWifiOnly,
    lanVisibility,
    autoApplySyncedClip,
    config,
    syncRestarting,
    supabaseUrl,
    setSupabaseUrl,
    supabaseKey,
    setSupabaseKey,
    supabaseEmail,
    setSupabaseEmail,
    supabasePassword,
    setSupabasePassword,
    relayUrl,
    setRelayUrl,
    savedMsg,
    saveError,
    testMsg,
    testing,
    passphrase,
    setPassphrase,
    passphraseSavedMsg,
    passphraseSaveOk,
    syncStatus,
    // Shortcuts
    pendingShortcut,
    setPendingShortcut,
    currentShortcut,
    defaultShortcut,
    shortcutMsg,
    // Storage
    maxTextBytes,
    setMaxTextBytes,
    maxImageBytes,
    setMaxImageBytes,
    maxFileBytes,
    setMaxFileBytes,
    quotaBytes,
    setQuotaBytes,
    sensitiveTtlSecs,
    setSensitiveTtlSecs,
    exportInProgress,
    exportMsg,
    exportIncludeSensitive,
    setExportIncludeSensitive,
    importInProgress,
    importMsg,
    importPending,
    setImportPending,
    dbStats,
    vacuumBusy,
    vacuumMsg,
    deleteMsg,
    limitsMsg,
    // Global
    loadState,
    degradedReason,
    staleDaemon,
    setReloadKey,
    // Derived
    offline: loadState !== "ready",
    degraded: loadState === "degraded",
    notReady: loadState === "not_ready",
    // CopyPaste-yw2k: set cloudAccountMismatch=true when ANY paired peer carries
    // a supabase_account_id that differs from our own local one. Keep false when:
    //   - local id is null/absent (cloud-sync off or not signed in)
    //   - peer has no id (legacy build or cloud-sync not configured on peer)
    //   - all peers with ids match the local id
    // This drives the CloudAccountMismatchBanner already wired in SettingsView.
    localSupabaseAccountId: syncStatus?.supabase_account_id ?? null,
    cloudAccountMismatch,
    // Helpers (functions read live state via closure)
    buildConfigPatch,
    showLimitsMsg,
    saveLimitsField,
    addExcludedApp,
    removeExcludedApp,
    deleteConfirm,
    setDeleteConfirm,
    // Handlers
    handlePrivateMode,
    handleAllowScreenshots,
    handleSaveConfig,
    handleTestConnection,
    handleSyncEnabledToggle,
    handleP2pToggle,
    handleWifiOnlyToggle,
    handleLanVisibilityToggle,
    handleAutoApplySyncedClipToggle,
    handleSaveShortcut,
    handleResetShortcut,
    handleSetPassphrase,
    handleDeleteAll,
    handleVacuum,
    handleExport,
    handleImportFile,
    handleConfirmImport,
  };
}
