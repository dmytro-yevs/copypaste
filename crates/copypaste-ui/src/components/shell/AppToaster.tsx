import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Toaster } from "sonner";

import { isAndroidPlatform } from "@/lib/platform";

type ResolvedTheme = "dark" | "light";

function themeOnDocument(): ResolvedTheme {
  return document.documentElement.dataset.theme === "light" ? "light" : "dark";
}

/** The Sonner portal is a body child, while the desktop content pane is not.
 * Measure the actual pane so a resized or redesigned sidebar cannot displace
 * feedback into the wrong visual centre. */
function useContentPaneCenter(enabled: boolean) {
  useLayoutEffect(() => {
    const root = document.documentElement;
    if (!enabled) {
      root.style.removeProperty("--content-pane-center");
      return;
    }

    const pane = document.querySelector<HTMLElement>("[data-content-pane]");
    if (!pane) return;
    const update = () => {
      const bounds = pane.getBoundingClientRect();
      root.style.setProperty("--content-pane-center", `${bounds.left + bounds.width / 2}px`);
    };
    update();
    const observer = typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(update);
    observer?.observe(pane);
    window.addEventListener("resize", update);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", update);
      root.style.removeProperty("--content-pane-center");
    };
  }, [enabled]);
}

/**
 * Sonner owns a portal outside the application shell, so it cannot inherit
 * the shell's theme class. Read the resolved document theme instead: system
 * appearance changes and the preference both update that single attribute.
 */
export function AppToaster() {
  const [theme, setTheme] = useState<ResolvedTheme>(themeOnDocument);
  const android = isAndroidPlatform();
  const toaster = useRef<HTMLElement>(null);
  useContentPaneCenter(!android);
  const mobileOffset = {
    top: "calc(var(--inset-top) + var(--s-3))",
    right: "calc(var(--inset-right) + var(--s-3))",
    bottom: "calc(var(--tabbar-h) + var(--inset-bottom) + var(--s-3))",
    left: "calc(var(--inset-left) + var(--s-3))",
  };

  useEffect(() => {
    const root = document.documentElement;
    const observer = new MutationObserver(() => setTheme(themeOnDocument()));
    observer.observe(root, { attributes: true, attributeFilter: ["data-theme"] });
    setTheme(themeOnDocument());
    return () => observer.disconnect();
  }, []);

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
      // Android needs this even on a wide device, where Sonner does not apply
      // `mobileOffset` but CopyPaste still has its bottom navigation.
      offset={
        android
          ? mobileOffset
          : { bottom: "calc(var(--ctl-h-sm) + var(--s-3))" }
      }
      mobileOffset={mobileOffset}
      toastOptions={{ className: "font-sans" }}
    />
  );
}
