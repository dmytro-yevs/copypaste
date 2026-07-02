// SyncTab.tsx
// Extracted from SettingsView.tsx renderSync() (CopyPaste-g06m.14 split) — cut/paste only.
import { SectionHeader } from "../../../components/SectionHeader";
import { AlertTriangle, Check, Key, Plug, Save } from "lucide-react";
import { SettingsRow } from "../../../components/SettingsRow";
import { Toggle } from "../../../components/Toggle";
import { Panel } from "../../../components/Panel";
import { InfoPopover } from "../components/InfoPopover";
import { CloudAccountMismatchBanner } from "../components/CloudAccountMismatchBanner";
import { LimitsMsg } from "../components/LimitsMsg";
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
  passphraseSaveOk: boolean;
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
  passphraseSaveOk: _passphraseSaveOk,
  testMsg,
  testing,
  savedMsg,
  saveError,
  syncStatus,
  limitsMsg,
  inputCls: _inputCls,
  btnCls: _btnCls,
  btnStyle: _btnStyle,
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
  // Q2: one consolidated status instead of 3 raw status rows.
  const syncBlocker =
    syncStatus === null
      ? null
      : !syncStatus.passphrase_set
        ? "Sync passphrase not set"
        : syncStatus.supabase_configured && !syncStatus.signed_in
          ? "Not signed in to your cloud account"
          : null;

  return (
    <div>
      {/* crh3.15: single canonical signed-in banner — surface-card only.
          The former raw bg-ide-success/5 div duplicated this when signed_in && email;
          merged into one element so only one status row ever renders. */}
      {syncStatus !== null &&
        syncStatus.supabase_configured &&
        syncStatus.signed_in &&
        syncStatus.email && (
          <div className="statusrow mb-4">
            <span>Signed in as {syncStatus.email}</span>
            <span className="txt-faint">— All devices must use this same account to sync.</span>
          </div>
        )}

      {/* Q2: single consolidated sync status banner. */}
      {syncStatus !== null && (
        syncBlocker === null ? (
          <div className="banner banner--ok">
            <Check aria-hidden="true" />
            <span className="banner__x">
              Sync ready{syncStatus.last_sync_ms ? ` · last synced ${formatLastSync(syncStatus.last_sync_ms)}` : ""}
            </span>
          </div>
        ) : (
          <div className="banner banner--warn">
            <AlertTriangle aria-hidden="true" />
            <span className="banner__x">{syncBlocker}</span>
          </div>
        )
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
          <div className="ctl">
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
          <div className="ctl">
            <LimitsMsg field="auto_apply_synced_clip" limitsMsg={limitsMsg} />
            <Toggle
              checked={autoApplySyncedClip}
              onChange={(v) => void handleAutoApplySyncedClipToggle(v)}
              disabled={offline || !syncEnabled}
              aria-label="Auto-apply synced clipboard"
            />
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
          <div className="ctl">
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
          <div className="ctl">
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
          {/* g27b.32: field--grow-full fills the available card width (see
              primitives.css) — the bare .field sat at the browser's ~146px
              intrinsic input width regardless of card width. */}
          <div className="field field--grow-full">
            <input
              type="url"
              placeholder="https://your-project.supabase.co"
              value={supabaseUrl}
              onChange={(e) => setSupabaseUrl(e.target.value)}
              disabled={offline}
              autoComplete="off"
              spellCheck={false}
            />
          </div>
        </SettingsRow>
        {/* bdac.80: standardized to "Anon key" (sentence case, drop redundant "Supabase" prefix — section header already provides context) */}
        <SettingsRow title="Anon key">
          {/* g27b.32: ctl--grow + field--grow-full — see Supabase URL above. */}
          <div className="ctl ctl--grow">
            <div className="field field--grow-full">
              <input
                type="password"
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
            </div>
            {syncStatus?.supabase_configured && !supabaseKey && (
              <span className="statusrow">set ✓</span>
            )}
          </div>
        </SettingsRow>
        {/* jhvl: Supabase GoTrue email + password for email+password sign-in.
             These are WRITE-ONLY — the daemon never returns them; only the
             supabase_email_set / supabase_password_set presence flags come back.
             Inputs are cleared after a successful Save. Password is always masked. */}
        {/* crh3.17: "Email" matches "Anon key" pattern (no prefix; bdac.80) */}
        <SettingsRow title="Email">
          {/* g27b.32: ctl--grow + field--grow-full — see Supabase URL above. */}
          <div className="ctl ctl--grow">
            <div className="field field--grow-full">
              <input
                type="email"
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
            </div>
            {syncStatus?.supabase_email_set && !supabaseEmail && (
              <span className="statusrow">set ✓</span>
            )}
          </div>
        </SettingsRow>
        {/* crh3.17: "Password" matches "Anon key" / "Email" pattern (no prefix) */}
        <SettingsRow title="Password">
          {/* g27b.32: ctl--grow + field--grow-full — see Supabase URL above. */}
          <div className="ctl ctl--grow">
            <div className="field field--grow-full">
              <input
                type="password"
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
            </div>
            {syncStatus?.supabase_password_set && !supabasePassword && (
              <span className="statusrow">set ✓</span>
            )}
          </div>
        </SettingsRow>
        {/* bdac.104: InfoPopover moved to info= slot (label column) */}
        <SettingsRow
          title="Relay URL"
          info={<InfoPopover text="Optional HTTP relay for store-and-forward sync when devices aren't on the same network. Leave blank to use direct P2P / cloud sync only. Saved with the cloud-sync settings." />}
        >
          {/* g27b.32: field--grow-full — see Supabase URL above. */}
          <div className="field field--grow-full">
            <input
              type="url"
              placeholder="https://relay.example.com"
              value={relayUrl}
              onChange={(e) => setRelayUrl(e.target.value)}
              disabled={offline}
              autoComplete="off"
              spellCheck={false}
            />
          </div>
        </SettingsRow>
        {/* bdac.104: InfoPopover moved to info= slot (label column) */}
        <SettingsRow
          title="Sync passphrase"
          info={<InfoPopover text="Enter the same passphrase on every device to enable encrypted sync. Click 'Set passphrase' or press Enter to save." />}
        >
          <div className="ctl ctl--col">
            <div className="ctl">
              <div className="field">
                <input
                  type="password"
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
              </div>
              <button
                type="button"
                className="btn btn--secondary sm"
                disabled={offline || passphrase.trim() === ""}
                onClick={() => void handleSetPassphrase()}
              ><Key aria-hidden="true" />Set passphrase</button>
            </div>
            {passphraseSavedMsg !== null && (
              <span className="field-note field-note--dim">
                {passphraseSavedMsg}
              </span>
            )}
          </div>
        </SettingsRow>
        {testMsg !== null && (
          <div className={` mt-3`}>
            <span className="banner__x">{testMsg.text}</span>
          </div>
        )}
        <div className="ctl ctl--wide mt-4">
          {saveError !== null && (
            <span className="field-note field-note--err">{saveError}</span>
          )}
          {savedMsg && !saveError && (
            <span className="field-note field-note--ok">Saved</span>
          )}
          <button
            type="button"
            className="btn btn--secondary sm"
            disabled={offline || testing}
            onClick={() => void handleTestConnection()}
          >
            <Plug aria-hidden="true" />{testing ? "Testing…" : "Test connection"}
          </button>
          <button
            type="button"
            className="btn btn--primary sm"
            disabled={offline}
            onClick={() => void handleSaveConfig()}
          ><Save aria-hidden="true" />Save</button>
        </div>
      </Panel>
    </div>
  );
}
