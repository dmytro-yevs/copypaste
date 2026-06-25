// ShortcutsTab.tsx
// Extracted from SettingsView.tsx renderShortcuts() (CopyPaste-g06m.14 split) — cut/paste only.
// bdac.59: The "Shortcuts" tab is macOS-only. Android has no equivalent because
// global keyboard shortcuts are not available on Android (no system-level hotkey
// registration API). If Android gains a quick-paste gesture/shortcut in the future,
// a corresponding settings entry should be added to Android's SettingsActivity.
import { Panel } from "../../../components/Panel";
import { SettingsRow } from "../../../components/SettingsRow";
import { InfoPopover } from "../components/InfoPopover";
import { ShortcutCapture } from "../components/ShortcutCapture";

export type ShortcutsTabProps = {
  pendingShortcut: string;
  setPendingShortcut: (v: string) => void;
  currentShortcut: string;
  defaultShortcut: string;
  shortcutMsg: { text: string; isError: boolean } | null;
  btnCls: string;
  btnStyle: React.CSSProperties;
  handleResetShortcut: () => void;
  handleSaveShortcut: () => void;
};

export function ShortcutsTab({
  pendingShortcut,
  setPendingShortcut,
  currentShortcut,
  defaultShortcut,
  shortcutMsg,
  btnCls,
  btnStyle,
  handleResetShortcut,
  handleSaveShortcut,
}: ShortcutsTabProps) {
  return (
    <div className="space-y-2">
      <Panel>
        {/* bdac.104: InfoPopover moved to info= slot (label column) */}
        <SettingsRow
          title="Open popup"
          info={<InfoPopover text="Click then press a combo. OS-reserved keys (Cmd+Space etc.) cannot be overridden." />}
        >
          <div className="flex flex-col items-end gap-1">
            <div className="flex items-center gap-2">
              <ShortcutCapture
                value={pendingShortcut}
                onChange={setPendingShortcut}
              />
              <button
                type="button"
                aria-label="Reset shortcut to default"
                title={`Reset to default (${defaultShortcut})`}
                disabled={
                  currentShortcut === defaultShortcut &&
                  pendingShortcut === defaultShortcut
                }
                onClick={() => void handleResetShortcut()}
                className="flex h-7 w-7 items-center justify-center border border-ide-border bg-ide-elevated text-ide-dim hover:bg-ide-hover hover:text-ide-text disabled:cursor-not-allowed disabled:opacity-40 transition-colors"
                style={{ borderRadius: "var(--skin-r-ctl)" }}
              >
                <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="M2.5 8a5.5 5.5 0 1 1 1.6 3.9" />
                  <path d="M2.5 12v-3.2h3.2" />
                </svg>
              </button>
              <button
                type="button"
                disabled={pendingShortcut === currentShortcut}
                onClick={() => void handleSaveShortcut()}
                className={btnCls}
                style={btnStyle}
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
          </div>
        </SettingsRow>
      </Panel>
    </div>
  );
}
