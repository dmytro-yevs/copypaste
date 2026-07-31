/**
 * INV-20: the shell is **never** inside an error boundary — navigation and the
 * main pane get sibling boundaries, so a crash in a screen cannot take
 * navigation with it (CopyPaste-8ebg.12).
 */
import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useEventListener } from "usehooks-ts";

import { BannerBar } from "@/components/shell/Banners";
import { Boundary } from "@/components/shell/Boundary";
import { Sidebar } from "@/components/shell/Sidebar";
import { CaptureSetup } from "@/components/capture/CaptureSetup";
import { DevicesView } from "@/components/devices/DevicesView";
import { HistoryView } from "@/components/history/HistoryView";
import { SettingsView } from "@/components/settings/SettingsView";
import { useCaptureSync } from "@/hooks/useCapture";
import { useStatus } from "@/hooks/useHistory";
import { usePush } from "@/hooks/usePush";
import { useTranslation } from "@/i18n";
import { legacyHistoryPresent } from "@/lib/banners";
import { classifyError } from "@/lib/errors";
import {
  CURRENT_PROTOCOL_VERSION,
  hideWindow,
  setAllowScreenshots,
} from "@/lib/ipc";
import { applyAppearance, subscribeSystemTheme } from "@/lib/theme";
import { selectAppearance, usePrefs } from "@/store/prefs";
import { useShallow } from "zustand/react/shallow";
import { useUi } from "@/store/ui";

const SCREENS = {
  history: {
    label: "nav.history",
    render: (pushLive: boolean) => <HistoryView pushLive={pushLive} />,
  },
  devices: { label: "nav.devices", render: () => <DevicesView /> },
  settings: { label: "nav.settings", render: () => <SettingsView /> },
  capture: { label: "capture.title", render: () => <CaptureSetup /> },
} as const;

export default function App() {
  const { t } = useTranslation();
  const view = useUi((s) => s.view);
  // `useShallow` is load-bearing: without it this is a render loop that
  // unmounts the app — 55 renders in 2.5s, measured.
  const appearance = usePrefs(useShallow(selectAppearance));
  const allowScreenshots = usePrefs((s) => s.allowScreenshots);
  const status = useStatus();
  const qc = useQueryClient();
  // Both subscribed once, here, not per screen: two subscribers invalidate the
  // same queries twice for one change.
  const pushLive = usePush();
  useCaptureSync();

  // Subscribes *once*: v1 accumulated a matchMedia listener per re-apply
  // (CopyPaste-g27b.20).
  useEffect(() => {
    applyAppearance(appearance);
    subscribeSystemTheme(() => applyAppearance(usePrefs.getState()));
  }, [appearance]);

  // INV-35. The window is already protected, so this only ever *relaxes* it and
  // a failure leaves the user protected — which is why revealing a secret needs
  // no ordering against it.
  useEffect(() => {
    void setAllowScreenshots(allowScreenshots).catch(() => {});
  }, [allowScreenshots]);

  // B-20: for a popover summoned by a hotkey, Escape *is* the dismissal.
  // `defaultPrevented` is the whole test for "something nearer the key already
  // used it" — Radix's dismissable layer and the search field both set it.
  useEventListener("keydown", (event) => {
    if (event.key !== "Escape" || event.defaultPrevented) return;
    // INV-25: through the backend, never `window.hide()`. A build with no
    // window to hide — the browser, a test — is not a failure.
    void hideWindow().catch(() => {});
  });

  const statusKind = status.error ? classifyError(status.error) : null;
  const screen = SCREENS[view];

  return (
    // `flex-col-reverse` puts the nav at the bottom of a phone screen — the
    // reachable band — while keeping it first in the DOM for reading order.
    <div className="flex h-full min-h-0 flex-col-reverse bg-background text-foreground sm:flex-row">
      <Boundary label={t("shell.boundary.navigation")}>
        <Sidebar />
      </Boundary>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <BannerBar
          conditions={{
            serviceOffline: statusKind === "offline",
            historyUnreadable:
              statusKind === "legacy_database" || statusKind === "key_unusable"
                ? statusKind
                : null,
            protocolMismatch:
              status.data !== undefined &&
              status.data.protocol_version !== CURRENT_PROTOCOL_VERSION
                ? status.data.protocol_version
                : null,
            capturePaused: status.data?.capture_running === false,
            legacyHistory: legacyHistoryPresent(status.data),
          }}
          onRetry={() => void qc.invalidateQueries()}
        />

        <Boundary label={t(screen.label)}>
          {/* `display: contents`: the boundary must not join the flex height
              chain the scroll regions depend on. */}
          <div className="contents">{screen.render(pushLive)}</div>
        </Boundary>
      </div>
    </div>
  );
}
