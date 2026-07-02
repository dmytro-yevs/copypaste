import { useState } from "react";
import { TabBar, type TabId } from "../../SettingsView/components/TabBar";
import { SettingsRow } from "../../../components/SettingsRow";
import { Toggle } from "../../../components/Toggle";

// Task 6.7: "settings tab/row". A minimal standalone demo — not the full
// SettingsView (which is wired to live IPC state) — of TabBar + SettingsRow,
// the two primitives that compose every Settings tab.
export function SettingsSection() {
  const [tab, setTab] = useState<TabId>("general");
  const [soundOn, setSoundOn] = useState(true);
  return (
    <section id="gallery-settings">
      <h2>Settings tab · row</h2>
      <TabBar active={tab} onChange={setTab} />
      <div className="set-body">
        <div className="set-pane on">
          <SettingsRow
            title="Play sound on copy"
            description="Plays a soft sound when an item is copied."
          >
            <Toggle checked={soundOn} onChange={setSoundOn} aria-label="Play sound on copy" />
          </SettingsRow>
        </div>
      </div>
    </section>
  );
}
