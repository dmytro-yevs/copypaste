import { invoke } from "@tauri-apps/api/core";

import type { ResolvedTheme } from "@/lib/theme";

function hasNativeBridge(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** The platform shell is not part of the DOM, so it needs this explicit sync. */
export function applyNativeAppearance(theme: ResolvedTheme): void {
  if (!hasNativeBridge()) return;
  void invoke("set_native_theme", { theme }).catch(() => {});
}
