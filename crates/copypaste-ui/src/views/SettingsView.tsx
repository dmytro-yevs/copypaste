// SettingsView.tsx — thin composition root (CopyPaste-g06m.35)
// State/effects/handlers: SettingsView/hooks/useSettingsState.ts
// Tabs: SettingsView/tabs/  Banners: SettingsView/components/
import { useState, useEffect } from "react";
import { ConfirmModal } from "../components/ConfirmModal";
import { ViewShell } from "../components/ViewShell";
import { ToastProvider, useToast } from "../components/Toast";
import { useSettingsState, BTN_CLS, BTN_STYLE, INPUT_CLS } from "./SettingsView/hooks/useSettingsState";
import { TabBar, type TabId } from "./SettingsView/components/TabBar";
import { StatusBanners } from "./SettingsView/components/StatusBanners";
import { GeneralTab } from "./SettingsView/tabs/GeneralTab";
import { DisplayTab } from "./SettingsView/tabs/DisplayTab";
import { SyncTab } from "./SettingsView/tabs/SyncTab";
import { ShortcutsTab } from "./SettingsView/tabs/ShortcutsTab";
import { StorageTab, type StorageTabProps } from "./SettingsView/tabs/StorageTab";
import { AboutContent } from "./AboutView";
import { LogContent } from "./LogView";

// Route settings save/action confirmations to the shared bottom-centre toast —
// per the design request, "saved / done" notices are a popup at the bottom of
// the app, not inline text. Each transition to a truthy message shows one toast.
function SettingsToaster({ s }: { s: ReturnType<typeof useSettingsState> }) {
  const { show } = useToast();
  useEffect(() => { if (s.savedMsg) show("Settings saved", { kind: "success" }); }, [s.savedMsg, show]);
  useEffect(() => { if (s.passphraseSavedMsg) show(s.passphraseSavedMsg, { kind: "success" }); }, [s.passphraseSavedMsg, show]);
  useEffect(() => { if (s.shortcutMsg) show(s.shortcutMsg.text, { kind: s.shortcutMsg.isError ? "error" : "success" }); }, [s.shortcutMsg, show]);
  useEffect(() => { if (s.exportMsg) show(s.exportMsg.text, { kind: s.exportMsg.isError ? "error" : "success" }); }, [s.exportMsg, show]);
  useEffect(() => { if (s.importMsg) show(s.importMsg.text, { kind: s.importMsg.isError ? "error" : "success" }); }, [s.importMsg, show]);
  useEffect(() => { if (s.vacuumMsg) show(s.vacuumMsg.text, { kind: s.vacuumMsg.isError ? "error" : "success" }); }, [s.vacuumMsg, show]);
  useEffect(() => { if (s.deleteMsg) show(s.deleteMsg.text, { kind: s.deleteMsg.isError ? "error" : "success" }); }, [s.deleteMsg, show]);
  useEffect(() => { if (s.saveError) show(s.saveError, { kind: "error" }); }, [s.saveError, show]);
  return null;
}

export function SettingsView() {
  const [activeTab, setActiveTab] = useState<TabId>("general");
  // CopyPaste-8ebg.54: lifted out of LogContent so the log filter survives
  // switching to another Settings tab and back (the `{activeTab === "logs" &&
  // <LogContent/>}` pane below fully unmounts LogContent on every tab
  // switch — this is the minimal, tractable fix; keeping every Settings tab
  // permanently mounted to avoid ANY state loss is a bigger layout/perf
  // change, deferred — see bd note on CopyPaste-8ebg.54).
  const [logFilter, setLogFilter] = useState("");
  const s = useSettingsState();
  const onRetry = () => s.setReloadKey((k) => k + 1);

  return (
    <ToastProvider>
    <ViewShell title="Settings">
      <SettingsToaster s={s} />
      <div className="set-banners">
        <StatusBanners loadState={s.loadState} staleDaemon={s.staleDaemon} degradedReason={s.degradedReason} onRetry={onRetry} />
      </div>

      {s.loadState === "loading" && (
        <div>Loading…</div>
      )}

      {s.loadState !== "loading" && (
        <div className="fill-col">
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
            {activeTab === "about" && (
              <div className="set-pane on" role="tabpanel" id="tabpanel-about" aria-labelledby="tab-about">
                <AboutContent />
              </div>
            )}
            {activeTab === "logs" && (
              <div className="set-pane on" role="tabpanel" id="tabpanel-logs" aria-labelledby="tab-logs">
                <LogContent filter={logFilter} onFilterChange={setLogFilter} />
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
    </ToastProvider>
  );
}

export type { TabId };
