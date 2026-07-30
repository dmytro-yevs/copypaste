/**
 * INV-20: the shell is **never** inside an error boundary — navigation and the
 * main pane get sibling boundaries, so a crash in a screen cannot take
 * navigation with it and the fallback renders inside the layout rather than
 * against a bare body (CopyPaste-8ebg.12).
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

  // main.tsx owns the first frame; this keeps the attributes in step, and
  // subscribes *once* — v1 accumulated a matchMedia listener per re-apply
  // (CopyPaste-g27b.20).
  useEffect(() => {
    applyAppearance(appearance);
    subscribeSystemTheme(() => applyAppearance(usePrefs.getState()));
  }, [appearance]);

  const statusKind = status.error ? classifyError(status.error) : null;
  const screen = SCREENS[view];

  return (
    // `flex-col-reverse` puts the nav at the bottom of a phone screen — the
    // reachable band — while keeping it first in the DOM for reading order.
    <div className="flex h-full min-h-0 flex-col-reverse bg-background text-foreground sm:flex-row">
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
          {/* `display: contents`: the boundary must not join the flex height
              chain the scroll regions depend on. */}
          <div className="contents">{screen.render()}</div>
        </Boundary>
      </div>
    </div>
  );
}
