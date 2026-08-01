/**
 * INV-20: the shell is **never** inside an error boundary — navigation and the
 * main pane get sibling boundaries, so a crash in a screen cannot take
 * navigation with it (CopyPaste-8ebg.12).
 */
import { lazy, Suspense, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";

import { BannerBar } from "@/components/shell/Banners";
import { Boundary } from "@/components/shell/Boundary";
import { Sidebar } from "@/components/shell/Sidebar";
import { HistoryView } from "@/components/history/HistoryView";
import { useCaptureState, useCaptureSync } from "@/hooks/useCapture";
import { useStatus } from "@/hooks/useHistory";
import { usePush } from "@/hooks/usePush";
import { useTranslation } from "@/i18n";
import { classifyError } from "@/lib/errors";
import { hasBridge, setAllowScreenshots } from "@/lib/ipc";
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
  const setView = useUi((s) => s.setView);
  // `useShallow` is load-bearing: without it this is a render loop that
  // unmounts the app — 55 renders in 2.5s, measured.
  const appearance = usePrefs(useShallow(selectAppearance));
  const allowScreenshots = usePrefs((s) => s.allowScreenshots);
  const status = useStatus();
  // Both subscribed once, here, not per screen: two subscribers invalidate the
  // same queries twice for one change.
  const pushLive = usePush();
  useCaptureSync();
  const capture = useCaptureState();
  const welcomed = useRef(false);

  useEffect(() => {
    const global = window as typeof window & { __copypasteRequestedView?: string };
    const openSettings = () => {
      useUi.getState().setView("settings");
      delete global.__copypasteRequestedView;
    };
    if (
      new URLSearchParams(window.location.search).get("view") === "settings" ||
      global.__copypasteRequestedView === "settings"
    ) {
      openSettings();
    }
    window.addEventListener("copypaste:open-settings", openSettings);
    return () => window.removeEventListener("copypaste:open-settings", openSettings);
  }, []);

  useEffect(() => {
    if (!hasBridge()) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen("open-settings", () => useUi.getState().setView("settings")).then((detach) => {
      if (cancelled) detach();
      else unlisten = detach;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!hasBridge()) return;
    let unlisten: (() => void) | undefined;
    const receive = (urls: string[]) => {
      const pairingUri = urls.find((url) => {
        try {
          const parsed = new URL(url);
          return parsed.protocol === "copypaste:" && parsed.hostname === "pair";
        } catch {
          return false;
        }
      });
      if (pairingUri) {
        useUi.getState().setPairingUri(pairingUri);
        useUi.getState().setView("devices");
      }
    };
    void getCurrent().then((urls) => urls && receive(urls)).catch(() => {});
    void onOpenUrl(receive).then((detach) => {
      unlisten = detach;
    }).catch(() => {});
    return () => unlisten?.();
  }, []);

  // Subscribes *once*: v1 accumulated a matchMedia listener per re-apply
  // (CopyPaste-g27b.20).
  useEffect(() => {
    // Android has no window behind the activity to blur. Keep its app surface
    // solid even if this preference travelled through a shared native store.
    const apply = () =>
      applyAppearance({
        ...usePrefs.getState(),
        translucency: isAndroidPlatform() ? 0 : usePrefs.getState().translucency,
      });
    apply();
    subscribeSystemTheme(apply);
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

  useEffect(() => {
    if (
      android &&
      !welcomed.current &&
      capture.data !== undefined &&
      capture.data.health.state !== "working"
    ) {
      welcomed.current = true;
      setView("capture");
    }
  }, [android, capture.data, setView]);

  return (
    <div
      className={cn(
        "app-surface flex h-full min-h-0 text-foreground",
        android ? "flex-col-reverse" : "app-surface--desktop flex-row",
      )}
    >
      <Boundary label={t("shell.boundary.navigation")}>
        <Sidebar />
      </Boundary>

      <main data-content-pane className="flex min-h-0 min-w-0 flex-1 flex-col">
        <BannerBar
          conditions={{
            historyUnreadable: statusKind === "key_unusable" ? statusKind : null,
          }}
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
