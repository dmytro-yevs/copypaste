/**
 * The app shell: nav rail, banner slot, and the active screen.
 *
 * INV-20 — the shell itself is **never** inside an error boundary. Navigation
 * and the main pane each get their own sibling boundary, so a crash in a screen
 * cannot take navigation with it and every fallback renders inside the shell
 * layout rather than against a bare document body (CopyPaste-8ebg.12).
 *
 * Routing is in-memory (zustand), with defensive narrowing: an unrecognised
 * view resolves to History rather than to a blank pane (manifest §3.0). There
 * is no view transition animation — v1's crossfade was deliberately stripped
 * (CopyPaste-h1n3) and the flex height chain the scroll regions depend on is
 * what remains of it.
 */
import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { BannerBar } from "@/components/shell/Banners";
import { Boundary } from "@/components/shell/Boundary";
import { Sidebar } from "@/components/shell/Sidebar";
import { DevicesView } from "@/components/devices/DevicesView";
import { HistoryView } from "@/components/history/HistoryView";
import { SettingsView } from "@/components/settings/SettingsView";
import { useStatus } from "@/hooks/useHistory";
import { classifyError } from "@/lib/errors";
import { CURRENT_PROTOCOL_VERSION } from "@/lib/ipc";
import { applyAppearance, subscribeSystemTheme } from "@/lib/theme";
import { selectAppearance, usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";

const SCREENS = {
  history: { label: "History", render: () => <HistoryView /> },
  devices: { label: "Devices", render: () => <DevicesView /> },
  settings: { label: "Settings", render: () => <SettingsView /> },
} as const;

export default function App() {
  const view = useUi((s) => s.view);
  const appearance = usePrefs(selectAppearance);
  const status = useStatus();
  const qc = useQueryClient();

  // The pre-paint script in main.tsx owns the *first* frame; this keeps the
  // attributes in step afterwards, and subscribes once to the OS appearance so
  // a `system` theme follows it live without a reload (AT-53).
  useEffect(() => {
    applyAppearance(appearance);
    subscribeSystemTheme(() => applyAppearance(usePrefs.getState()));
  }, [appearance]);

  const statusKind = status.error ? classifyError(status.error) : null;
  const screen = SCREENS[view];

  return (
    <div className="flex h-full min-h-0 bg-background text-foreground">
      <Boundary label="Navigation">
        <Sidebar />
      </Boundary>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <BannerBar
          conditions={{
            serviceOffline: statusKind === "offline",
            protocolMismatch:
              status.data !== undefined &&
              status.data.protocol_version !== CURRENT_PROTOCOL_VERSION
                ? status.data.protocol_version
                : null,
            capturePaused: status.data?.capture_running === false,
          }}
          onRetry={() => void qc.invalidateQueries()}
        />

        <Boundary label={screen.label}>
          {/* `display: contents` keeps the boundary out of the flex height
              chain the scroll regions depend on. */}
          <div className="contents">{screen.render()}</div>
        </Boundary>
      </div>
    </div>
  );
}
