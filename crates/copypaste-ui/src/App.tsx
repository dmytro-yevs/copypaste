/**
 * INV-20: the shell is **never** inside an error boundary — navigation and the
 * main pane get sibling boundaries, so a crash in a screen cannot take
 * navigation with it (CopyPaste-8ebg.12).
 */
import { lazy, Suspense, useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { BannerBar } from "@/components/shell/Banners";
import { Boundary } from "@/components/shell/Boundary";
import { Sidebar } from "@/components/shell/Sidebar";
import { HistoryView } from "@/components/history/HistoryView";
import { useCaptureSync } from "@/hooks/useCapture";
import { useStatus } from "@/hooks/useHistory";
import { usePush } from "@/hooks/usePush";
import { useTranslation } from "@/i18n";
import { legacyHistoryPresent } from "@/lib/banners";
import { classifyError } from "@/lib/errors";
import {
  CURRENT_PROTOCOL_VERSION,
  setAllowScreenshots,
} from "@/lib/ipc";
import { applyAppearance, subscribeSystemTheme } from "@/lib/theme";
import { cn } from "@/lib/cn";
import { isAndroidPlatform } from "@/lib/platform";
import { selectAppearance, usePrefs } from "@/store/prefs";
import { useShallow } from "zustand/react/shallow";
import { useUi } from "@/store/ui";

const DevicesView = lazy(async () => ({
  default: (await import("@/components/devices/DevicesView")).DevicesView,
}));
const SettingsView = lazy(async () => ({
  default: (await import("@/components/settings/SettingsView")).SettingsView,
}));
const CaptureSetup = lazy(async () => ({
  default: (await import("@/components/capture/CaptureSetup")).CaptureSetup,
}));

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

  const statusKind = status.error ? classifyError(status.error) : null;
  const screen = SCREENS[view];
  const android = isAndroidPlatform();

  return (
    <div
      className={cn(
        "flex h-full min-h-0 bg-background text-foreground",
        android ? "flex-col-reverse" : "flex-row",
      )}
    >
      <Boundary label={t("shell.boundary.navigation")}>
        <Sidebar />
      </Boundary>

      <main className="flex min-h-0 min-w-0 flex-1 flex-col">
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
          <Suspense
            fallback={<div className="flex min-h-0 flex-1" role="status" />}
          >
            <div className="contents">{screen.render(pushLive)}</div>
          </Suspense>
        </Boundary>
      </main>
    </div>
  );
}
