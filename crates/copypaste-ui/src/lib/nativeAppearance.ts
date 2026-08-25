import { UI_COMMANDS } from "@/generated/ipc";
import { call, hasNativeBridge } from "@/lib/ipcCall";
import type { ResolvedTheme } from "@/lib/theme";

/** The platform shell is not part of the DOM, so it needs this explicit sync. */
export function applyNativeAppearance(theme: ResolvedTheme): void {
  if (!hasNativeBridge()) return;
  void call(UI_COMMANDS.set_native_theme, { theme }).catch(() => {});
}
