import { invoke } from "@tauri-apps/api/core";

import type { ResolvedTheme } from "@/lib/theme";

interface SystemAccent {
  color: string;
}

function hasNativeBridge(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** The platform shell is not part of the DOM, so it needs this explicit sync. */
export function applyNativeAppearance(theme: ResolvedTheme): void {
  if (!hasNativeBridge()) return;
  void invoke("set_native_theme", { theme }).catch(() => {});
}

/** Uses the platform colour in native builds and preserves a stable browser fallback. */
export async function applySystemAccent(): Promise<void> {
  if (!hasNativeBridge()) return;
  const accent = await invoke<SystemAccent>("system_accent").catch(() => null);
  if (!accent || !/^#[0-9a-f]{6}$/i.test(accent.color)) return;

  const root = document.documentElement;
  if (root.dataset.accent !== "system") return;
  const color = accent.color.toLowerCase();
  const onAccent = readableText(color);
  root.style.setProperty("--accent", color);
  root.style.setProperty("--accent-2", color);
  root.style.setProperty("--on-accent", onAccent);
  root.style.setProperty("--accent-away", "#000000");
  root.style.setProperty("--system-accent-preview", color);
  root.style.setProperty("--system-on-accent", onAccent);
}

function readableText(hex: string): "#ffffff" | "#111111" {
  const [red, green, blue] = [1, 3, 5].map((offset) => {
    const channel = Number.parseInt(hex.slice(offset, offset + 2), 16) / 255;
    return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue > 0.179 ? "#111111" : "#ffffff";
}
