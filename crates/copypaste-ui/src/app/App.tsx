/**
 * INV-20: the shell is **never** inside an error boundary — navigation and the
 * main pane get sibling boundaries, so a crash in a screen cannot take
 * navigation with it (CopyPaste-8ebg.12).
 */
import { useEffect, useState } from "react";

import { ApplicationShell } from "@/app/shell";
import { OnboardingScreen } from "@/features/onboarding";
import { useCaptureState, useCaptureSync } from "@/hooks/useCapture";
import { useInboundPairingNav } from "@/features/pairing";
import { statusReachable, useStatus } from "@/hooks/useStatus";
import { usePush } from "@/hooks/usePush";
import { useSizeClass } from "@/hooks/useSizeClass";
import { classifyError } from "@/lib/errors";
import { setAllowScreenshots } from "@/lib/ipc";
import { isAndroidPlatform } from "@/lib/platform";
import { subscribeNativeEvent } from "@/lib/tauriEventRegistry";
import { EVENT_OPEN_SETTINGS } from "@/lib/tauriEvents";
import { applyAppearance, subscribeSystemTheme } from "@/lib/theme";
import { selectAppearance, usePrefs } from "@/store/prefs";
import { usePrefsHydrated } from "@/store/prefsHydrated";
import { useUi } from "@/store/ui";
import { useShallow } from "zustand/react/shallow";

export default function App() {
  const setView = useUi((s) => s.setView);
  // `useShallow` is load-bearing: without it this is a render loop that
  // unmounts the app — 55 renders in 2.5s, measured.
  const appearance = usePrefs(useShallow(selectAppearance));
  const allowScreenshots = usePrefs((s) => s.allowScreenshots);
  // Only `status.error` is read here, and the root re-rendering twice a second
  // re-renders every screen under it.
  const status = useStatus(statusReachable);
  // Both subscribed once, here, not per screen: two subscribers invalidate the
  // same queries twice for one change.
  const pushLive = usePush();
  useCaptureSync();
  useInboundPairingNav();
  const capture = useCaptureState();
  const [androidStartupSettled, setAndroidStartupSettled] = useState(false);
  const prefsHydrated = usePrefsHydrated();
  const onboardingComplete = usePrefs((s) => s.onboardingComplete);
  const onboardingOpen = useUi((s) => s.onboardingOpen);
  const showOnboarding =
    prefsHydrated && (!onboardingComplete || onboardingOpen);

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
    return subscribeNativeEvent(EVENT_OPEN_SETTINGS, () => {
      useUi.getState().setView("settings");
    });
  }, []);

  // Subscribes *once*: v1 accumulated a matchMedia listener per re-apply
  // (CopyPaste-g27b.20).
  useEffect(() => {
    // Android has no window behind the activity to blur. Keep its app surface
    // solid even if this preference travelled through a shared native store.
    const apply = () =>
      applyAppearance({
        ...usePrefs.getState(),
        translucency: isAndroidPlatform() ? false : usePrefs.getState().translucency,
      });
    apply();
    return subscribeSystemTheme(apply);
  }, [appearance]);

  // INV-35. The window is already protected, so this only ever *relaxes* it and
  // a failure leaves the user protected — which is why revealing a secret needs
  // no ordering against it.
  useEffect(() => {
    void setAllowScreenshots(allowScreenshots).catch(() => {});
  }, [allowScreenshots]);

  const statusKind = status.error ? classifyError(status.error) : null;
  const android = isAndroidPlatform();
  const sizeClass = useSizeClass();
  const navigationReady = !android || androidStartupSettled;

  useEffect(() => {
    if (
      showOnboarding ||
      !android ||
      androidStartupSettled ||
      (capture.data === undefined && !capture.isError)
    ) {
      return;
    }
    if (
      capture.data !== undefined &&
      capture.data.health.state !== "working" &&
      useUi.getState().view === "history"
    ) {
      setView("capture");
    }
    setAndroidStartupSettled(true);
  }, [android, androidStartupSettled, capture.data, capture.isError, setView, showOnboarding]);

  if (showOnboarding) {
    return <OnboardingScreen data-size-class={sizeClass} />;
  }

  return <ApplicationShell navigationReady={navigationReady} pushLive={pushLive} statusKind={statusKind} />;
}
