// ShortcutsTab.tsx
// Extracted from SettingsView.tsx renderShortcuts() (CopyPaste-g06m.14 split) — cut/paste only.
// bdac.59: The "Shortcuts" tab is macOS-only. Android has no equivalent because
// global keyboard shortcuts are not available on Android (no system-level hotkey
// registration API). If Android gains a quick-paste gesture/shortcut in the future,
// a corresponding settings entry should be added to Android's SettingsActivity.
import { RotateCcw } from "lucide-react";
import { SectionHeader } from "../../../components/SectionHeader";
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
  btnCls: _btnCls,
  btnStyle: _btnStyle,
  handleResetShortcut,
  handleSaveShortcut,
}: ShortcutsTabProps) {
  return (
    <div>
      {/* Design-reference parity: this group is labelled "Global shortcuts".
          CopyPaste-f72f: this tab has exactly one row (bdac.59: macOS-only,
          no Android equivalent to add rows for), which read as an empty
          panel floating in `.set-body`'s flex:1 space. `hint` (existing
          SectionHeader prop, .srow__s) adds a one-line explanation so the
          sparse layout reads as "intentionally minimal" rather than
          "broken/missing content" — no new components or CSS needed. */}
      <SectionHeader
        label="Global shortcuts"
        hint="macOS-only. Opens the quick-paste popup from anywhere, even when CopyPaste isn't focused."
      />
      <Panel>
        {/* bdac.104: InfoPopover moved to info= slot (label column) */}
        <SettingsRow
          title="Open popup"
          info={<InfoPopover text="Click then press a combo. OS-reserved keys (Cmd+Space etc.) cannot be overridden." />}
        >
          <div className="ctl ctl--col">
            <div className="ctl">
              <ShortcutCapture
                value={pendingShortcut}
                onChange={setPendingShortcut}
              />
              <button
                type="button"
                className="iconbtn"
                aria-label="Reset shortcut to default"
                title={`Reset to default (${defaultShortcut})`}
                disabled={
                  currentShortcut === defaultShortcut &&
                  pendingShortcut === defaultShortcut
                }
                onClick={() => void handleResetShortcut()}
              >
                <RotateCcw aria-hidden="true" />
              </button>
              <button
                type="button"
                className="btn btn--primary sm"
                disabled={pendingShortcut === currentShortcut}
                onClick={() => void handleSaveShortcut()}
              >
                Save
              </button>
            </div>
            {shortcutMsg !== null && (
              <span className={`field-note `}>
                {shortcutMsg.text}
              </span>
            )}
          </div>
        </SettingsRow>
      </Panel>
    </div>
  );
}
