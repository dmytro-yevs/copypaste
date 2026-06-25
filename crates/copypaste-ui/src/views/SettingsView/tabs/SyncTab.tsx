// SyncTab.tsx
// Extracted from SettingsView.tsx renderSync() (CopyPaste-g06m.14 split) — cut/paste only.
import { Check, X } from "lucide-react";
import { SectionHeader } from "../../../components/SectionHeader";
import { SettingsRow } from "../../../components/SettingsRow";
import { Toggle } from "../../../components/Toggle";
import { Panel } from "../../../components/Panel";
import { InfoPopover } from "../components/InfoPopover";
import { StatusRow } from "../components/StatusRow";
import { CloudAccountMismatchBanner } from "../components/CloudAccountMismatchBanner";
import { formatSyncTime } from "../../../lib/time";
import type { SyncStatus, AppSettings } from "../../../lib/ipc";

// i2sr (PG-40): delegate to the shared formatSyncTime hybrid formatter so
// SettingsView and DeviceCard use the same relative/absolute boundary (24 h).
// Input is ms (SyncStatus.last_sync_ms). Returns "Never" for null/0.
function formatLastSync(ms: number | null): string {
  if (ms === null) return "Never";
  return formatSyncTime(ms, "ms") ?? "Never";
}

export type SyncTabProps = {
  offline: boolean;
  syncEnabled: boolean;
  syncOnWifiOnly: boolean;
  autoApplySyncedClip: boolean;
  config: Pick<AppSettings, "p2p_enabled">;
  syncRestarting: boolean;
  lanVisibility: boolean;
  supabaseUrl: string;
  setSupabaseUrl: (v: string) => void;
  supabaseKey: string;
  setSupabaseKey: (v: string) => void;
  supabaseEmail: string;
  setSupabaseEmail: (v: string) => void;
  supabasePassword: string;
  setSupabasePassword: (v: string) => void;
  relayUrl: string;
  setRelayUrl: (v: string) => void;
  passphrase: string;
  setPassphrase: (v: string) => void;
  passphraseSavedMsg: string | null;
  testMsg: { text: string; ok: boolean } | null;
  testing: boolean;
  savedMsg: boolean;
  saveError: string | null;
  syncStatus: SyncStatus | null;
  limitsMsg: Record<string, { ok: boolean; message: string } | null>;
  inputCls: string;
  btnCls: string;
  btnStyle: React.CSSProperties;
  handleWifiOnlyToggle: (v: boolean) => void;
  handleAutoApplySyncedClipToggle: (v: boolean) => void;
  handleP2pToggle: (v: boolean) => void;
  handleLanVisibilityToggle: (v: boolean) => void;
  handleSetPassphrase: () => void;
  handleTestConnection: () => void;
  handleSaveConfig: () => void;
  /**
   * CopyPaste-1jms.34: true when a cross-device Supabase account mismatch is
   * detected. Controls visibility of the CloudAccountMismatchBanner.
   *
   * Until peer supabase_account_id is plumbed (CopyPaste-1jms.35), callers
   * pass false here — the banner is intentionally hidden to avoid false positives.
   */
  cloudAccountMismatch: boolean;
  /**
   * This device's canonical Supabase account id, for informational display in
   * the mismatch banner. Null/absent when cloud-sync is off or anon-key-only.
   */
  localSupabaseAccountId?: string | null;
};

// bdac.106: branch on .ok (typed signal) — no string comparison.
function LimitsMsg({ field, limitsMsg }: { field: string; limitsMsg: Record<string, { ok: boolean; message: string } | null> }) {
  const entry = limitsMsg[field];
  if (!entry) return null;
  return (
    <span className={`text-[11px] ${entry.ok ? "text-ide-success" : "text-ide-danger"}`}>
      {entry.message}
    </span>
  );
}

export function SyncTab({
  offline,
  syncEnabled,
  syncOnWifiOnly,
  autoApplySyncedClip,
  config,
  syncRestarting,
  lanVisibility,
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
  passphrase,
  setPassphrase,
  passphraseSavedMsg,
  testMsg,
  testing,
  savedMsg,
  saveError,
  syncStatus,
  limitsMsg,
  inputCls,
  btnCls,
  btnStyle,
  handleWifiOnlyToggle,
  handleAutoApplySyncedClipToggle,
  handleP2pToggle,
  handleLanVisibilityToggle,
  handleSetPassphrase,
  handleTestConnection,
  handleSaveConfig,
  cloudAccountMismatch,
  localSupabaseAccountId,
}: SyncTabProps) {
  return (
    <div className="space-y-2">
      {/* Status banner */}
      {syncStatus !== null && syncStatus.supabase_configured && (
        <div className="border border-ide-success/30 bg-ide-success/5 px-3 py-2 text-[13px] text-ide-success" style={{ borderRadius: "var(--skin-r-ctl)" }}>
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
          <div className="surface-card px-3 py-2 text-[13px] text-ide-dim" style={{ borderRadius: "var(--skin-r-ctl)" }}>
            <span className="font-medium text-ide-text">Signed in as {syncStatus.email}</span>
            <span className="ml-1">— All devices must use this same account to sync.</span>
          </div>
        )}

      {/* ── General sync ── */}
      {/* bdac.78: "Sync on Wi-Fi only" and "Auto-apply synced clipboard" apply
          to ALL transports (P2P + cloud), not just P2P — moved here from the
          P2P sub-section to match the canonical grouping:
            (1) General — wifi-only, auto-apply
            (2) Sync (LAN) — p2p_enabled, LAN visibility
            (3) Cloud sync — credentials
          j9xj (PG-30): per-transport controls are visually disabled when the
          master syncEnabled kill-switch is off (they still show their state). */}
      <SectionHeader
        label="General sync"
        hint="Applies to all sync transports."
      />
      <Panel>
        <SettingsRow title="Sync on Wi-Fi only">
          <div className="flex items-center gap-2">
            <LimitsMsg field="sync_on_wifi_only" limitsMsg={limitsMsg} />
            <Toggle
              checked={syncOnWifiOnly}
              onChange={(v) => void handleWifiOnlyToggle(v)}
              disabled={offline || !syncEnabled}
            />
          </div>
        </SettingsRow>
        {/* bdac.104: InfoPopover moved to info= slot (label column) */}
        <SettingsRow
          title="Auto-apply synced clipboard"
          info={<InfoPopover text="When on, incoming synced items from other devices are automatically written to the local clipboard so it stays up-to-date. When off, synced items are saved to history but never applied to the active clipboard — paste manually from the history list." />}
        >
          <div className="flex flex-col items-end gap-1">
            {/* wrfv: visible inline notice so the user knows synced clips will
                silently overwrite the active clipboard (the actual write is
                daemon-side; this is the UI surface for the setting). */}
            {autoApplySyncedClip && (
              <span
                data-testid="auto-apply-notice"
                role="note"
                className="text-[11px] text-ide-faint"
              >
                Synced clips will overwrite the active clipboard automatically.
                Turn off to keep clipboard intact and paste manually from history.
              </span>
            )}
            <div className="flex items-center gap-1.5">
              <LimitsMsg field="auto_apply_synced_clip" limitsMsg={limitsMsg} />
              <Toggle
                checked={autoApplySyncedClip}
                onChange={(v) => void handleAutoApplySyncedClipToggle(v)}
                disabled={offline || !syncEnabled}
                aria-label="Auto-apply synced clipboard"
              />
            </div>
          </div>
        </SettingsRow>
      </Panel>

      {/* ── Sync (LAN) ── */}
      <SectionHeader
        label="Sync (LAN)"
        hint="Same network, no account needed."
      />
      <Panel>
        {/* bdac.44: InfoPopover added — P2P row had no description */}
        {/* bdac.104: InfoPopover moved to info= slot (label column) */}
        <SettingsRow
          title="Enable P2P (LAN) sync"
          info={<InfoPopover text="Direct device-to-device sync over your local network. Requires a paired device on the Devices screen. Disable for cloud-only sync." />}
        >
          <div className="flex items-center gap-2">
            <LimitsMsg field="p2p_enabled" limitsMsg={limitsMsg} />
            <Toggle
              checked={config.p2p_enabled}
              onChange={(v) => void handleP2pToggle(v)}
              disabled={offline || syncRestarting || !syncEnabled}
              aria-label="P2P sync"
            />
          </div>
        </SettingsRow>
        {/* bdac.104: InfoPopover moved to info= slot (label column) */}
        <SettingsRow
          title="Visible on local network"
          info={<InfoPopover text="When off, this device stops advertising via mDNS-SD and will not appear in the device list on other Macs on the same network. Paired peers with a known address can still connect directly." />}
        >
          <div className="flex items-center gap-1.5">
            <LimitsMsg field="lan_visibility" limitsMsg={limitsMsg} />
            <Toggle
              checked={lanVisibility}
              onChange={(v) => void handleLanVisibilityToggle(v)}
              disabled={offline || !syncEnabled}
              aria-label="LAN visibility"
            />
          </div>
        </SettingsRow>
      </Panel>

      {/* ── Cloud sync ── */}
      <SectionHeader
        label="Cloud sync"
        hint="Syncs over the internet via your Supabase project."
      />
      {/* CopyPaste-1jms.34: cross-device Supabase account mismatch banner.
          Renders only when cloudAccountMismatch is true. Until peer account ids
          are plumbed (CopyPaste-1jms.35) callers always pass false, so the
          banner is hidden and no false positives are shown. */}
      <CloudAccountMismatchBanner
        hasMismatch={cloudAccountMismatch}
        localAccountId={localSupabaseAccountId}
      />
      <Panel>
        <SettingsRow title="Supabase URL">
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
        {/* bdac.80: standardized to "Anon key" (sentence case, drop redundant "Supabase" prefix — section header already provides context) */}
        <SettingsRow title="Anon key">
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
        {/* jhvl: Supabase GoTrue email + password for email+password sign-in.
             These are WRITE-ONLY — the daemon never returns them; only the
             supabase_email_set / supabase_password_set presence flags come back.
             Inputs are cleared after a successful Save. Password is always masked. */}
        <SettingsRow title="Supabase email">
          <div className="flex flex-col items-end gap-0.5">
            <input
              type="email"
              className={inputCls}
              placeholder={
                syncStatus?.supabase_email_set && !supabaseEmail
                  ? "set ✓ (leave blank to keep)"
                  : "user@example.com"
              }
              value={supabaseEmail}
              onChange={(e) => setSupabaseEmail(e.target.value)}
              disabled={offline}
              autoComplete="username"
              spellCheck={false}
            />
            {syncStatus?.supabase_email_set && !supabaseEmail && (
              <span className="text-[11px] text-ide-success">set ✓</span>
            )}
          </div>
        </SettingsRow>
        <SettingsRow title="Supabase password">
          <div className="flex flex-col items-end gap-0.5">
            <input
              type="password"
              className={inputCls}
              placeholder={
                syncStatus?.supabase_password_set && !supabasePassword
                  ? "set ✓ (leave blank to keep)"
                  : "Password"
              }
              value={supabasePassword}
              onChange={(e) => setSupabasePassword(e.target.value)}
              disabled={offline}
              autoComplete="current-password"
              spellCheck={false}
            />
            {syncStatus?.supabase_password_set && !supabasePassword && (
              <span className="text-[11px] text-ide-success">set ✓</span>
            )}
          </div>
        </SettingsRow>
        {/* bdac.104: InfoPopover moved to info= slot (label column) */}
        <SettingsRow
          title="Relay URL"
          info={<InfoPopover text="Optional HTTP relay for store-and-forward sync when devices aren't on the same network. Leave blank to use direct P2P / cloud sync only. Saved with the cloud-sync settings." />}
        >
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
        </SettingsRow>
        {/* bdac.104: InfoPopover moved to info= slot (label column) */}
        <SettingsRow
          title="Sync passphrase"
          info={<InfoPopover text="Enter the same passphrase on every device to enable encrypted sync. Click 'Set passphrase' or press Enter to save." />}
        >
          <div className="flex flex-col items-end gap-1">
            <div className="flex items-center gap-1.5">
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
                className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
                style={{ borderRadius: "var(--skin-r-ctl)" }}
              >
                Set passphrase
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
          </div>
        </SettingsRow>
        {testMsg !== null && (
          <div
            className={[
              "border-t border-ide-divider px-3 py-2 text-[12px] flex items-center gap-1.5",
              testMsg.ok ? "text-ide-success" : "text-ide-danger",
            ].join(" ")}
          >
            {/* §6.6: replaced ✓/✗ text chars with Lucide icons (size 14) */}
            {testMsg.ok ? <Check size={14} /> : <X size={14} />}
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
            style={btnStyle}
          >
            {testing ? "Testing…" : "Test connection"}
          </button>
          <button
            type="button"
            disabled={offline}
            onClick={() => void handleSaveConfig()}
            className={btnCls}
            style={btnStyle}
          >
            Save
          </button>
        </div>
      </Panel>

      {/* Sync status detail */}
      {syncStatus !== null && (
        <>
          <SectionHeader label="Status" />
          <Panel>
            <div className="px-3 py-2 space-y-1">
              <StatusRow label="Passphrase set" ok={syncStatus.passphrase_set} />
              <StatusRow label="Supabase configured" ok={syncStatus.supabase_configured} />
              <StatusRow label="Signed in" ok={syncStatus.signed_in} />
              {/* i2sr (PG-40): hybrid relative/absolute format + "Synced " prefix
                  to match Android parity. Relative when ≤24 h ago; absolute beyond. */}
              <div className="flex items-center gap-2 text-[13px] text-ide-dim pt-0.5">
                <span className="w-[140px] shrink-0">Last sync</span>
                <span className="text-ide-text">
                  {syncStatus.last_sync_ms
                    ? `Synced ${formatLastSync(syncStatus.last_sync_ms)}`
                    : "Never"}
                </span>
              </div>
            </div>
          </Panel>
        </>
      )}
    </div>
  );
}
