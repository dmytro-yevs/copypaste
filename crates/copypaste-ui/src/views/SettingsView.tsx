// SettingsView.tsx — thin composition root (CopyPaste-g06m.35)
// State/effects/handlers: SettingsView/hooks/useSettingsState.ts
// Tabs: SettingsView/tabs/  Banners: SettingsView/components/
import { useState } from "react";
import { ConfirmModal } from "../components/ConfirmModal";
import { ViewShell } from "../components/ViewShell";
import { useSettingsState, BTN_CLS, BTN_STYLE, INPUT_CLS } from "./SettingsView/hooks/useSettingsState";
import { TabBar, type TabId } from "./SettingsView/components/TabBar";
import { StatusBanners } from "./SettingsView/components/StatusBanners";
import { GeneralTab } from "./SettingsView/tabs/GeneralTab";
import { DisplayTab } from "./SettingsView/tabs/DisplayTab";
import { SyncTab } from "./SettingsView/tabs/SyncTab";
import { ShortcutsTab } from "./SettingsView/tabs/ShortcutsTab";
import { StorageTab, type StorageTabProps } from "./SettingsView/tabs/StorageTab";

export function SettingsView() {
  const [activeTab, setActiveTab] = useState<TabId>("general");
  const s = useSettingsState();
  const onRetry = () => s.setReloadKey((k) => k + 1);

  return (
    <ViewShell title="Settings">
      <StatusBanners loadState={s.loadState} staleDaemon={s.staleDaemon} degradedReason={s.degradedReason} onRetry={onRetry} />

      {s.loadState === "loading" && (
        <div>Loading…</div>
      )}

      {s.loadState !== "loading" && (
        <div>
          {/* TabBar itself carries .set-tabs (shell.css: the flex row that
              directly parents the .set-tab buttons) — see TabBar.tsx. */}
          <TabBar active={activeTab} onChange={setActiveTab} />
          <div className="set-body">
            {activeTab === "general" && (
              <div className="set-pane on" role="tabpanel" id="tabpanel-general" aria-labelledby="tab-general">
                <GeneralTab
                  offline={s.offline} loadState={s.loadState} prefs={s.prefs} setPrefs={s.setPrefs}
                  syncEnabled={s.syncEnabled} syncEnabledStub={s.syncEnabledStub}
                  privateMode={s.privateMode} privateModeError={s.privateModeError}
                  notifPermDenied={s.notifPermDenied} collectPublicIp={s.collectPublicIp}
                  setCollectPublicIp={s.setCollectPublicIp} pasteAsPlainText={s.pasteAsPlainText}
                  setPasteAsPlainText={s.setPasteAsPlainText} allowScreenshots={s.allowScreenshots}
                  allowScreenshotsError={s.allowScreenshotsError} excludedApps={s.excludedApps}
                  newExcludedApp={s.newExcludedApp} setNewExcludedApp={s.setNewExcludedApp}
                  daemonVersion={s.daemonVersion} limitsMsg={s.limitsMsg}
                  buildConfigPatch={s.buildConfigPatch} handleSyncEnabledToggle={s.handleSyncEnabledToggle}
                  handlePrivateMode={s.handlePrivateMode} handleAllowScreenshots={s.handleAllowScreenshots}
                  addExcludedApp={s.addExcludedApp} removeExcludedApp={s.removeExcludedApp}
                  setReloadKey={s.setReloadKey}
                />
              </div>
            )}
            {activeTab === "display" && (
              <div className="set-pane on" role="tabpanel" id="tabpanel-display" aria-labelledby="tab-display">
                <DisplayTab prefs={s.prefs} setPrefs={s.setPrefs} />
              </div>
            )}
            {activeTab === "sync" && (
              <div className="set-pane on" role="tabpanel" id="tabpanel-sync" aria-labelledby="tab-sync">
                <SyncTab
                  offline={s.offline} syncEnabled={s.syncEnabled} syncOnWifiOnly={s.syncOnWifiOnly}
                  autoApplySyncedClip={s.autoApplySyncedClip} config={s.config}
                  syncRestarting={s.syncRestarting} lanVisibility={s.lanVisibility}
                  supabaseUrl={s.supabaseUrl} setSupabaseUrl={s.setSupabaseUrl}
                  supabaseKey={s.supabaseKey} setSupabaseKey={s.setSupabaseKey}
                  supabaseEmail={s.supabaseEmail} setSupabaseEmail={s.setSupabaseEmail}
                  supabasePassword={s.supabasePassword} setSupabasePassword={s.setSupabasePassword}
                  relayUrl={s.relayUrl} setRelayUrl={s.setRelayUrl}
                  passphrase={s.passphrase} setPassphrase={s.setPassphrase}
                  passphraseSavedMsg={s.passphraseSavedMsg} passphraseSaveOk={s.passphraseSaveOk} testMsg={s.testMsg} testing={s.testing}
                  savedMsg={s.savedMsg} saveError={s.saveError} syncStatus={s.syncStatus}
                  limitsMsg={s.limitsMsg} inputCls={INPUT_CLS} btnCls={BTN_CLS} btnStyle={BTN_STYLE}
                  handleWifiOnlyToggle={s.handleWifiOnlyToggle}
                  handleAutoApplySyncedClipToggle={s.handleAutoApplySyncedClipToggle}
                  handleP2pToggle={s.handleP2pToggle} handleLanVisibilityToggle={s.handleLanVisibilityToggle}
                  handleSetPassphrase={s.handleSetPassphrase} handleTestConnection={s.handleTestConnection}
                  handleSaveConfig={s.handleSaveConfig}
                  cloudAccountMismatch={s.cloudAccountMismatch}
                  localSupabaseAccountId={s.localSupabaseAccountId}
                />
              </div>
            )}
            {/* bdac.59: Shortcuts tab is macOS-only — no Android equivalent. */}
            {activeTab === "shortcuts" && (
              <div className="set-pane on" role="tabpanel" id="tabpanel-shortcuts" aria-labelledby="tab-shortcuts">
                <ShortcutsTab
                  pendingShortcut={s.pendingShortcut} setPendingShortcut={s.setPendingShortcut}
                  currentShortcut={s.currentShortcut} defaultShortcut={s.defaultShortcut}
                  shortcutMsg={s.shortcutMsg} btnCls={BTN_CLS} btnStyle={BTN_STYLE}
                  handleResetShortcut={s.handleResetShortcut} handleSaveShortcut={s.handleSaveShortcut}
                />
              </div>
            )}
            {activeTab === "storage" && (
              <div className="set-pane on" role="tabpanel" id="tabpanel-storage" aria-labelledby="tab-storage">
                <StorageTab
                  offline={s.offline} prefs={s.prefs} setPrefs={s.setPrefs}
                  maxTextBytes={s.maxTextBytes} setMaxTextBytes={s.setMaxTextBytes}
                  maxImageBytes={s.maxImageBytes} setMaxImageBytes={s.setMaxImageBytes}
                  maxFileBytes={s.maxFileBytes} setMaxFileBytes={s.setMaxFileBytes}
                  quotaBytes={s.quotaBytes} setQuotaBytes={s.setQuotaBytes}
                  sensitiveTtlSecs={s.sensitiveTtlSecs} setSensitiveTtlSecs={s.setSensitiveTtlSecs}
                  exportInProgress={s.exportInProgress} exportMsg={s.exportMsg}
                  exportIncludeSensitive={s.exportIncludeSensitive}
                  setExportIncludeSensitive={s.setExportIncludeSensitive}
                  importInProgress={s.importInProgress} importMsg={s.importMsg}
                  dbStats={s.dbStats} vacuumBusy={s.vacuumBusy} vacuumMsg={s.vacuumMsg}
                  deleteMsg={s.deleteMsg} limitsMsg={s.limitsMsg} btnCls={BTN_CLS} btnStyle={BTN_STYLE}
                  saveLimitsField={s.saveLimitsField as StorageTabProps["saveLimitsField"]}
                  showLimitsMsg={s.showLimitsMsg} handleExport={s.handleExport}
                  handleImportFile={s.handleImportFile} handleVacuum={s.handleVacuum}
                  setDeleteConfirm={s.setDeleteConfirm}
                />
              </div>
            )}
          </div>
        </div>
      )}

      {/* w6xc: Clear history confirmation modal */}
      <ConfirmModal
        open={s.deleteConfirm} title="Clear all clipboard history?"
        body="This will permanently delete all clipboard items stored on this device. This cannot be undone."
        confirmLabel="Clear history"
        onConfirm={() => { s.setDeleteConfirm(false); void s.handleDeleteAll(); }}
        onCancel={() => s.setDeleteConfirm(false)}
      />
      {/* vcnv: Import confirmation modal — bdac.73: "Import history" label */}
      <ConfirmModal
        open={s.importPending !== null} title="Import clipboard history?"
        body={`This will import ${s.importPending?.length ?? 0} item${(s.importPending?.length ?? 0) === 1 ? "" : "s"} from the file into your clipboard history. Duplicate items will be skipped. Existing items are not deleted.`}
        confirmLabel="Import"
        onConfirm={() => { void s.handleConfirmImport(); }}
        onCancel={() => s.setImportPending(null)}
      />
    </ViewShell>
  );
}

export type { TabId };
