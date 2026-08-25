import { useLayoutEffect, useRef, useSyncExternalStore } from "react";
import { Toaster } from "sonner";

import { useSizeClass } from "@/hooks/useSizeClass";
import { isAndroidPlatform } from "@/lib/platform";
import { isQuickPasteSurface } from "@/surface";
import {
  subscribeSystemTheme,
  systemThemeSnapshot,
  type ResolvedTheme,
} from "@/lib/theme";
import { usePrefs } from "@/store/prefs";
import "./AppToaster.module.css";

/**
 * Sonner owns a portal outside the application shell, so it cannot inherit
 * the shell's theme class. Read the resolved document theme instead: system
 * appearance changes and the preference both update that single attribute.
 */
export function AppToaster() {
  const themePref = usePrefs((state) => state.theme);
  const systemTheme = useSyncExternalStore(
    subscribeSystemTheme,
    systemThemeSnapshot,
    (): ResolvedTheme => "dark",
  );
  const theme: ResolvedTheme = themePref === "system" ? systemTheme : themePref;
  const android = isAndroidPlatform();
  const docked = useSizeClass() === "compact" && !isQuickPasteSurface(window.location.search);
  const toaster = useRef<HTMLElement>(null);
  const dockedOffset = {
    top: "calc(var(--inset-top) + var(--s-3))",
    right: "calc(var(--inset-right) + var(--s-3))",
    bottom: "calc(var(--tabbar-h) + var(--inset-bottom) + var(--s-3))",
    left: "calc(var(--inset-left) + var(--s-3))",
  };
  const paneOffset = { bottom: "calc(var(--ctl-h-sm) + var(--s-3))" };

  useLayoutEffect(() => {
    if (android) toaster.current?.setAttribute("aria-atomic", "true");
  }, [android]);

  return (
    <Toaster
      ref={toaster}
      position="bottom-center"
      theme={theme}
      closeButton
      richColors
      duration={3000}
      expand
      gap={8}
      visibleToasts={4}
      offset={docked ? dockedOffset : paneOffset}
      mobileOffset={docked ? dockedOffset : paneOffset}
      toastOptions={{ className: "font-sans" }}
    />
  );
}
