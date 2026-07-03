// GeneralTab.tsx
// Extracted from SettingsView.tsx renderGeneral() (CopyPaste-g06m.14 split) — cut/paste only.
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { SectionHeader } from "../../../components/SectionHeader";
import { Plus, X } from "lucide-react";
import { SettingsRow } from "../../../components/SettingsRow";
import { Toggle } from "../../../components/Toggle";
import { Panel } from "../../../components/Panel";
import { RestartDaemonButton } from "../../../components/RestartDaemonButton";
import { InfoPopover } from "../components/InfoPopover";
import { LimitsMsg } from "../components/LimitsMsg";
import { api, appVersion, invoke } from "../../../lib/ipc";
import type { AppSettings } from "../../../lib/ipc";
import type { UIPrefs } from "../../../store";

// CopyPaste-8ebg.53: reverse-DNS-ish bundle id, e.g. "com.1password.1password".
// Requires at least one dot and only the characters valid in a bundle
// identifier segment — rejects garbage like "not a bundle id" or "foo".
const BUNDLE_ID_PATTERN = /^[A-Za-z0-9]+(\.[A-Za-z0-9-]+)+$/;

export type GeneralTabProps = {
  offline: boolean;
  loadState: string;
  prefs: UIPrefs;
  setPrefs: (p: Partial<UIPrefs>) => void;
  syncEnabled: boolean;
  syncEnabledStub: boolean;
  privateMode: boolean;
  privateModeError: string | null;
  notifPermDenied: boolean;
  collectPublicIp: boolean;
  setCollectPublicIp: (v: boolean) => void;
  pasteAsPlainText: boolean;
  setPasteAsPlainText: (v: boolean) => void;
  allowScreenshots: boolean;
  allowScreenshotsError: string | null;
  excludedApps: string[];
  newExcludedApp: string;
  setNewExcludedApp: (v: string) => void;
  daemonVersion: string | null;
  limitsMsg: Record<string, { ok: boolean; message: string } | null>;
  buildConfigPatch: (overrides: Partial<AppSettings>) => AppSettings;
  handleSyncEnabledToggle: (v: boolean) => void;
  handlePrivateMode: (v: boolean) => void;
  handleAllowScreenshots: (v: boolean) => void;
  addExcludedApp: () => void;
  removeExcludedApp: (bundleId: string) => void;
  setReloadKey: React.Dispatch<React.SetStateAction<number>>;
};

export function GeneralTab({
  offline,
  loadState,
  prefs,
  setPrefs,
  syncEnabled,
  syncEnabledStub,
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
  limitsMsg,
  buildConfigPatch,
  handleSyncEnabledToggle,
  handlePrivateMode,
  handleAllowScreenshots,
  addExcludedApp,
  removeExcludedApp,
  setReloadKey,
}: GeneralTabProps) {
  // CopyPaste-8ebg.53: bundle-id inline validation — reverse-DNS pattern.
  const [bundleIdErr, setBundleIdErr] = useState<string | null>(null);
  const handleAddExcludedApp = () => {
    const id = newExcludedApp.trim();
    if (id === "") return;
    if (!BUNDLE_ID_PATTERN.test(id)) {
      setBundleIdErr('Enter a reverse-DNS bundle id, e.g. "com.example.app"');
      return;
    }
    setBundleIdErr(null);
    void addExcludedApp();
  };

  // CopyPaste-8ebg.63: the "Version" row's caption promises app + daemon
  // versions but only ever rendered the daemon's. Pull the app version the
  // same way AboutView does (bundle getVersion, IPC appVersion() fallback).
  const [appVer, setAppVer] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    getVersion().then(
      (v) => {
        if (!cancelled) setAppVer(v);
      },
      () => {
        void appVersion().then(
          (v) => {
            if (!cancelled && v) setAppVer(v);
          },
          () => {
            /* both unavailable — leave null, row just shows the daemon version */
          }
        );
      }
    );
    return () => {
      cancelled = true;
    };
  }, []);

  // CopyPaste-8ebg.53: launch_at_login defaulted to true with no UI control.
  // A get_launch_at_login/set_launch_at_login Tauri command pair was added
  // (src-tauri/src/config.rs, mirroring set_allow_screenshots) so this can be
  // wired directly via the shared `invoke` re-export without touching
  // useSettingsState.ts / tauriCommands.ts (outside this task's file scope).
  const [launchAtLogin, setLaunchAtLoginState] = useState<boolean | null>(null);
  const [launchAtLoginErr, setLaunchAtLoginErr] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    invoke<boolean>("get_launch_at_login")
      .then((v) => {
        if (!cancelled) setLaunchAtLoginState(v);
      })
      .catch(() => {
        /* command unavailable (older backend / mock mode) — row hides itself */
      });
    return () => {
      cancelled = true;
    };
  }, []);
  const handleLaunchAtLoginToggle = (v: boolean) => {
    setLaunchAtLoginState(v);
    setLaunchAtLoginErr(null);
    invoke<void>("set_launch_at_login", { enabled: v }).catch((e: unknown) => {
      setLaunchAtLoginState(!v);
      setLaunchAtLoginErr(e instanceof Error ? e.message : String(e));
    });
  };

  return (
    <div>
      {/* bdac.93: sub-group "General" — sync + private mode */}
      <SectionHeader label="General" />
      <Panel>
        {/* j9xj (PG-30): master sync kill-switch — Android parity.
            Daemon implements AppConfig::sync_enabled (tke7/PG-30).
            When off, visually gates per-transport switches in the Sync tab. */}
        {/* bdac.104: InfoPopover moved to info= slot (label column) for all rows */}
        <SettingsRow
          title="Enable sync"
          info={<InfoPopover text="Master switch for all sync transports (P2P, cloud, relay). When off, no data leaves this device. Matches Android sync_enabled parity. Configure P2P, cloud, and relay credentials on the Sync tab." />}
        >
          <div className="ctl">
            <LimitsMsg field="sync_enabled" limitsMsg={limitsMsg} />
            <Toggle
              checked={syncEnabled}
              onChange={(v) => void handleSyncEnabledToggle(v)}
              disabled={offline}
              aria-label="Enable sync"
            />
          </div>
        </SettingsRow>
        {/* 7set: full-width warning when the daemon doesn't acknowledge
            sync_enabled, so the user knows the toggle may have no effect. */}
        {syncEnabledStub && !offline && (
          <div className="field-note field-note--warn">
            Sync control unavailable — update the CopyPaste background service to enable this setting.
          </div>
        )}
        {/* bdac.47: InfoPopover added — Private mode had no description */}
        {/* bdac.107: Title Case — "Private Mode" matches all other row titles */}
        <SettingsRow
          title="Private Mode"
          info={<InfoPopover text="When on, this device stops recording new clipboard items and suppresses sync for the session. The notification's Pause action is a temporary per-session pause; Private Mode persists across restarts." />}
        >
          <div className="ctl">
            {privateModeError !== null && (
              <span className="field-note field-note--err">{privateModeError}</span>
            )}
            <Toggle
              checked={privateMode}
              onChange={(v) => void handlePrivateMode(v)}
              disabled={offline}
            />
          </div>
        </SettingsRow>
      </Panel>

      {/* bdac.93: sub-group "Notifications" — sound + notify-on-copy */}
      <SectionHeader label="Notifications" />
      <Panel>
        <SettingsRow title="Play sound on copy">
          <Toggle
            checked={prefs.playSoundOnCopy}
            onChange={(v) => {
              // P0 fix: only persist to daemon once settings are fully loaded.
              // buildConfigPatch reads hydrated slider/toggle state; calling it
              // before "ready" would push default values over the real config.
              setPrefs({ playSoundOnCopy: v });
              if (loadState === "ready") {
                void api.setConfig(buildConfigPatch({ sound_on_copy: v })).catch(() => {
                  setPrefs({ playSoundOnCopy: !v });
                });
              }
            }}
            disabled={offline}
          />
        </SettingsRow>
        <SettingsRow title="Show notification on copy">
          <div className="ctl">
            {/* vrur: warn when notify is enabled but OS has denied permission.
                Shows inline so the user can act without leaving Settings. */}
            {prefs.notifyOnCopy && notifPermDenied && (
              <span role="alert" className="field-note field-note--err">
                OS notification permission denied — notifications won't appear.
                Grant access in System Settings → Notifications.
              </span>
            )}
            <Toggle
              checked={prefs.notifyOnCopy}
              onChange={(v) => {
                // P0 fix: same guard as sound_on_copy above.
                setPrefs({ notifyOnCopy: v });
                if (loadState === "ready") {
                  void api.setConfig(buildConfigPatch({ notify_on_copy: v })).catch(() => {
                    setPrefs({ notifyOnCopy: !v });
                  });
                }
              }}
              disabled={offline}
            />
          </div>
        </SettingsRow>
      </Panel>

      {/* CopyPaste-8ebg.34: "Mask sensitive data" duplicated the Display tab's
          Privacy control (DisplayTab.tsx) — single source of truth kept there. */}
      <SectionHeader
        label="Capture"
        hint="Control public-IP lookup, paste formatting, and which apps are never captured."
      />
      <Panel>
        {/* bdac.104: InfoPopovers moved to info= slot (label column) */}
        <SettingsRow
          title="Discover public IP"
          info={<InfoPopover text="Allow a one-off STUN request to learn this device's public IP, shown in the device-info card. No data is sent to analytics." />}
        >
          <Toggle
            checked={collectPublicIp}
            onChange={(v) => {
              // Mirror sound/notify: persist only once fully loaded, revert on failure.
              setCollectPublicIp(v);
              if (loadState === "ready") {
                void api
                  .setConfig(buildConfigPatch({ collect_public_ip: v }))
                  .catch(() => setCollectPublicIp(!v));
              }
            }}
            disabled={offline}
          />
        </SettingsRow>
        {/* CMP-023: paste_as_plain_text is a macOS capture-path concept.
            Android has no parity yet (no analogous platform hook). */}
        {/* bdac.95: removed "macOS only; no Android parity" — Android also implements pasteAsPlainText */}
        <SettingsRow
          title="Paste as plain text"
          info={<InfoPopover text="Strip rich formatting (RTF/HTML) when pasting — writes plain text only." />}
        >
          <Toggle
            checked={pasteAsPlainText}
            onChange={(v) => {
              setPasteAsPlainText(v);
              if (loadState === "ready") {
                void api
                  .setConfig(buildConfigPatch({ paste_as_plain_text: v }))
                  .catch(() => setPasteAsPlainText(!v));
              }
            }}
            disabled={offline}
          />
        </SettingsRow>
        {/* CopyPaste-6uy9: allow-screenshots toggle. Tauri-direct (not daemon).
            Default = OFF (content protection ON = PG-25 behaviour). When enabled
            the NSWindow.sharingType is set to .readOnly so screenshots & screen
            recordings can capture CopyPaste windows. */}
        <SettingsRow
          title="Allow screenshots / screen recording"
          info={<InfoPopover text="When off (default), CopyPaste is excluded from screenshots and screen recordings (macOS NSWindowSharingNone / Android FLAG_SECURE). Enable only if you need to record or share your screen while using CopyPaste. The preference is applied immediately to all open windows." />}
        >
          <div className="ctl">
            {allowScreenshots && (
              <span role="note" className="field-note">
                Clipboard content may be captured by screenshots and screen recordings.
              </span>
            )}
            {allowScreenshotsError !== null && (
              <span className="field-note field-note--err">{allowScreenshotsError}</span>
            )}
            <Toggle
              checked={allowScreenshots}
              onChange={(v) => void handleAllowScreenshots(v)}
              aria-label="Allow screenshots and screen recording"
            />
          </div>
        </SettingsRow>
        {/* fullWidth: a label+info + input/add-button row + chip list doesn't
            fit the two-column SettingsRow layout — stacks title above the
            full-width control per SettingsRow's fullWidth contract. */}
        <SettingsRow
          title="Excluded apps"
          fullWidth
          info={<InfoPopover text="Bundle IDs of apps whose clipboard is never captured, e.g. com.1password.1password (macOS)." />}
        >
          <div className="ctl">
            <div className="field field--grow">
              <input
                type="text"
                value={newExcludedApp}
                placeholder="com.example.app"
                disabled={offline}
                onChange={(e) => {
                  setNewExcludedApp(e.target.value);
                  if (bundleIdErr !== null) setBundleIdErr(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    handleAddExcludedApp();
                  }
                }}
                aria-invalid={bundleIdErr !== null}
                /* audit P2: was bg-ide-bg (grey canvas) → looked disabled. Match
                   the Sync-tab text inputs: white/near-white elevated fill. */
              />
            </div>
            <button
              type="button"
              className="btn btn--primary sm"
              disabled={offline || newExcludedApp.trim() === ""}
              onClick={handleAddExcludedApp}
            ><Plus aria-hidden="true" />Add</button>
          </div>
          {bundleIdErr !== null && (
            <span className="field-note field-note--err">{bundleIdErr}</span>
          )}
          {excludedApps.length > 0 && (
            <div className="xapps">
              {excludedApps.map((bundleId) => (
                <div key={bundleId} className="xapp">
                  <span className="xapp__id" title={bundleId}>{bundleId}</span>
                  <button
                    type="button"
                    className="iconbtn danger"
                    aria-label={`Remove ${bundleId}`}
                    disabled={offline}
                    onClick={() => void removeExcludedApp(bundleId)}
                  >
                    <X aria-hidden="true" />
                  </button>
                </div>
              ))}
            </div>
          )}
        </SettingsRow>
      </Panel>

      <SectionHeader label="Background service" />
      <Panel>
        {/* bdac.107: description added for Version row (Background service section).
            CopyPaste-8ebg.63: the row's title implied both app + daemon version but
            only ever showed the daemon's — now shows both, app version first. */}
        <SettingsRow title="Version">
          <span className="field-note field-note--dim field-note--mono">
            App {appVer ?? "unknown"} · Daemon {offline ? "not running" : (daemonVersion ?? "unknown")}
          </span>
        </SettingsRow>
        {/* bdac.107: "Restart" → "Restart service" — unambiguous */}
        <SettingsRow title="Restart service">
          <RestartDaemonButton onRestarted={() => setReloadKey((k) => k + 1)} />
        </SettingsRow>
        {/* CopyPaste-8ebg.53: launch_at_login defaults to true with previously no
            UI control to disable it. Hidden (not disabled) when the backend
            command is unavailable (launchAtLogin stays null) rather than showing
            a toggle that silently does nothing. */}
        {launchAtLogin !== null && (
          <SettingsRow
            title="Launch at login"
            info={<InfoPopover text="Automatically start CopyPaste when you log in to macOS." />}
          >
            <div className="ctl">
              {launchAtLoginErr !== null && (
                <span className="field-note field-note--err">{launchAtLoginErr}</span>
              )}
              <Toggle
                checked={launchAtLogin}
                onChange={handleLaunchAtLoginToggle}
                aria-label="Launch at login"
              />
            </div>
          </SettingsRow>
        )}
      </Panel>
    </div>
  );
}
