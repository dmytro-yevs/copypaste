// GeneralTab.tsx
// Extracted from SettingsView.tsx renderGeneral() (CopyPaste-g06m.14 split) — cut/paste only.
import { SectionHeader } from "../../../components/SectionHeader";
import { SettingsRow } from "../../../components/SettingsRow";
import { Toggle } from "../../../components/Toggle";
import { Panel } from "../../../components/Panel";
import { RestartDaemonButton } from "../../../components/RestartDaemonButton";
import { InfoPopover } from "../components/InfoPopover";
import { LimitsMsg } from "../components/LimitsMsg";
import { api } from "../../../lib/ipc";
import type { AppSettings } from "../../../lib/ipc";
import type { UIPrefs } from "../../../store";

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
  return (
    <div className="space-y-2">
      {/* bdac.93: sub-group "General" — sync + private mode */}
      <SectionHeader label="General" />
      <Panel>
        {/* j9xj (PG-30): master sync kill-switch — Android parity.
            Daemon implements AppConfig::sync_enabled (tke7/PG-30).
            When off, visually gates per-transport switches in the Sync tab. */}
        {/* bdac.104: InfoPopover moved to info= slot (label column) for all rows */}
        <SettingsRow
          title="Enable sync"
          info={<InfoPopover text="Master switch for all sync transports (P2P, cloud, relay). When off, no data leaves this device. Matches Android sync_enabled parity." />}
        >
          <div className="flex flex-col items-end gap-1">
            {/* 7set: warn when daemon doesn't acknowledge sync_enabled so the
                user knows the toggle may have no effect on this daemon version. */}
            {syncEnabledStub && !offline && (
              <span className="text-[11px] text-ide-warning" role="note">
                Sync control unavailable — please update the CopyPaste background service to enable this setting.
              </span>
            )}
            <div className="flex items-center gap-1.5">
              <LimitsMsg field="sync_enabled" limitsMsg={limitsMsg} />
              <Toggle
                checked={syncEnabled}
                onChange={(v) => void handleSyncEnabledToggle(v)}
                disabled={offline}
                aria-label="Enable sync"
              />
            </div>
          </div>
        </SettingsRow>
        {/* bdac.47: InfoPopover added — Private mode had no description */}
        {/* bdac.107: Title Case — "Private Mode" matches all other row titles */}
        <SettingsRow
          title="Private Mode"
          info={<InfoPopover text="When on, this device stops recording new clipboard items and suppresses sync for the session. The notification's Pause action is a temporary per-session pause; Private Mode persists across restarts." />}
        >
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
          <div className="flex items-center gap-2">
            {/* vrur: warn when notify is enabled but OS has denied permission.
                Shows inline so the user can act without leaving Settings. */}
            {prefs.notifyOnCopy && notifPermDenied && (
              <span className="text-[11px] text-ide-warning" role="alert">
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

      {/* bdac.93: sub-group "Privacy" — mask sensitive. Named "Privacy" (not
          "Display") to avoid colliding with the "Display" appearance tab. */}
      <SectionHeader label="Privacy" />
      <Panel>
        {/* bdac.50: InfoPopover added for Mask sensitive data; bdac.104: moved to info= slot */}
        <SettingsRow
          title="Mask sensitive data"
          info={<InfoPopover text="Hide preview text for items flagged as sensitive (passwords, credit cards, tokens). Click an item in history to reveal its content." />}
        >
          <Toggle
            checked={prefs.maskSensitive}
            onChange={(v) => setPrefs({ maskSensitive: v })}
          />
        </SettingsRow>
      </Panel>

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
          <div className="flex flex-col items-end gap-1">
            {allowScreenshots && (
              <span className="text-[11px] text-ide-warning" role="note">
                Clipboard content may be captured by screenshots and screen recordings.
              </span>
            )}
            {allowScreenshotsError !== null && (
              <span className="text-[11px] text-ide-danger">{allowScreenshotsError}</span>
            )}
            <Toggle
              checked={allowScreenshots}
              onChange={(v) => void handleAllowScreenshots(v)}
              aria-label="Allow screenshots and screen recording"
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
              /* audit P2: was bg-ide-bg (grey canvas) → looked disabled. Match
                 the Sync-tab text inputs: white/near-white elevated fill. */
              className="flex-1 border border-ide-border bg-ide-elevated px-2.5 py-1.5 text-[13px] text-ide-text outline-none focus:border-ide-accent focus:ring-1 focus:ring-ide-accent disabled:cursor-not-allowed disabled:opacity-40"
              style={{ borderRadius: "var(--skin-r-ctl)" }}
            />
            <button
              type="button"
              disabled={offline || newExcludedApp.trim() === ""}
              onClick={() => void addExcludedApp()}
              className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
              style={{ borderRadius: "var(--skin-r-ctl)" }}
            >
              Add
            </button>
          </div>
          {excludedApps.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {excludedApps.map((bundleId) => (
                <span
                  key={bundleId}
                  className="inline-flex items-center gap-1 border border-ide-border bg-ide-elevated/40 px-2 py-1 text-[12px] text-ide-dim"
                  style={{ borderRadius: "var(--skin-r-ctl)" }}
                >
                  {bundleId}
                  <button
                    type="button"
                    aria-label={`Remove ${bundleId}`}
                    disabled={offline}
                    onClick={() => void removeExcludedApp(bundleId)}
                    className="flex h-6 w-6 items-center justify-center text-ide-faint hover:text-ide-danger disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    ×
                  </button>
                </span>
              ))}
            </div>
          )}
        </div>
      </Panel>

      <SectionHeader label="Background service" />
      <Panel>
        {/* bdac.107: description added for Version row (Background service section) */}
        <SettingsRow
          title="Version"
          description="Current daemon and app version."
        >
          <span className="text-[13px] text-ide-text">
            {offline ? "Not running" : (daemonVersion ?? "unknown")}
          </span>
        </SettingsRow>
        {/* bdac.107: "Restart" → "Restart service" — unambiguous; description added */}
        <SettingsRow
          title="Restart service"
          description="Restart the background clipboard service."
        >
          <RestartDaemonButton onRestarted={() => setReloadKey((k) => k + 1)} />
        </SettingsRow>
      </Panel>
    </div>
  );
}
