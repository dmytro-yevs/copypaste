/**
 * Global-shortcut capture (manifest §7.3).
 *
 * INV-23: the key comes from `event.code`, never `.key`, so a Cyrillic or
 * AZERTY layout records the same physical binding.
 */
import { t } from "@/i18n";
import { isWindowsPlatform } from "@/lib/platform";

export const DEFAULT_SHORTCUT = "CmdOrCtrl+Shift+V";

/**
 * Refused on macOS. `global-hotkey` binds ordinary keys through Carbon
 * `RegisterEventHotKey` (no permission needed) but falls back to an active
 * `CGEventTap` for exactly these five, which needs Accessibility — and per
 * ADR-0001 an ad-hoc-signed app loses that grant on every update, so the
 * shortcut would stop working after an upgrade.
 */
export const MEDIA_KEYS: readonly string[] = [
  "MediaPlayPause",
  "MediaTrackNext",
  "MediaTrackPrevious",
  "MediaFastForward",
  "MediaRewind",
];

/**
 * Windows binds media keys with `RegisterHotKey` and no permission at all, so
 * only the two that `global-hotkey`'s `key_to_vk` has no `VK_` constant for are
 * refused. `shell::hotkey::refusal` draws the same line; a mismatch would
 * either offer a key the backend rejects or hide one it would have taken.
 */
export const WINDOWS_UNBINDABLE_KEYS: readonly string[] = [
  "MediaFastForward",
  "MediaRewind",
];

const BARE_MODIFIERS = new Set(["Meta", "Control", "Alt", "Shift"]);

/** Key names Tauri spells its own way. */
const KEY_NAMES: Record<string, string> = {
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Space: "Space",
  " ": "Space",
  Enter: "Return",
  Return: "Return",
};

const PASS_THROUGH = new Set([
  "Escape",
  "Backspace",
  "Delete",
  "Tab",
  "Home",
  "End",
  "PageUp",
  "PageDown",
  ...Array.from({ length: 12 }, (_, i) => `F${i + 1}`),
]);

export type CaptureResult =
  | { readonly kind: "accelerator"; readonly value: string }
  | { readonly kind: "cancelled" }
  | { readonly kind: "refused"; readonly reason: string }
  | { readonly kind: "incomplete" };

function keyFrom(event: Pick<KeyboardEvent, "code" | "key">): string | null {
  const code = event.code || event.key;
  if (code.startsWith("Key")) return code.slice(3).toUpperCase();
  if (code.startsWith("Digit")) return code.slice(5);
  if (code in KEY_NAMES) return KEY_NAMES[code]!;
  if (PASS_THROUGH.has(code)) return code;
  if (MEDIA_KEYS.includes(code)) return code;
  if (code.length === 1) return code.toUpperCase();
  return code || null;
}

/**
 * Modifier order is fixed (`CmdOrCtrl`, `Alt`, `Shift`, key); `metaKey` and
 * `ctrlKey` both map to `CmdOrCtrl`, Tauri's cross-platform alias — which the
 * hotkey parser resolves to Command on macOS and Control everywhere else.
 */
export function captureAccelerator(
  event: Pick<KeyboardEvent, "code" | "key" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">,
  windows = isWindowsPlatform(),
): CaptureResult {
  if (event.key === "Escape" && !event.metaKey && !event.ctrlKey && !event.altKey) {
    return { kind: "cancelled" };
  }
  if (BARE_MODIFIERS.has(event.key)) return { kind: "incomplete" };

  const key = keyFrom(event);
  if (key === null) return { kind: "incomplete" };
  const refused = windows ? WINDOWS_UNBINDABLE_KEYS : MEDIA_KEYS;
  if (refused.includes(key)) {
    return {
      kind: "refused",
      reason: t(
        windows
          ? "settings.shortcut.refusal.noKeyCode"
          : "settings.shortcut.refusal.mediaKeys",
      ),
    };
  }

  const parts: string[] = [];
  if (event.metaKey || event.ctrlKey) parts.push("CmdOrCtrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");

  if (parts.length === 0) {
    return {
      kind: "refused",
      reason: t(
        windows
          ? "settings.shortcut.refusal.needsModifierWindows"
          : "settings.shortcut.refusal.needsModifier",
      ),
    };
  }

  parts.push(key);
  return { kind: "accelerator", value: parts.join("+") };
}

const MAC_GLYPHS: Record<string, string> = {
  CmdOrCtrl: "⌘",
  Cmd: "⌘",
  Command: "⌘",
  Meta: "⌘",
  Super: "⌘",
  Ctrl: "⌃",
  Control: "⌃",
  Alt: "⌥",
  Option: "⌥",
  Shift: "⇧",
  Return: "↩",
  Enter: "↩",
  Backspace: "⌫",
  Delete: "⌦",
  Escape: "⎋",
  Space: "␣",
  Tab: "⇥",
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
};

/**
 * A Windows keyboard has no ⌘, ⌥ or ⇧ engraved on it, and `CmdOrCtrl` is
 * registered there as **Control** — so the macOS table does not merely look
 * foreign, it names a different key from the one that is bound.
 */
const WINDOWS_GLYPHS: Record<string, string> = {
  CmdOrCtrl: "Ctrl",
  Cmd: "Win",
  Command: "Win",
  Meta: "Win",
  Super: "Win",
  Ctrl: "Ctrl",
  Control: "Ctrl",
  Alt: "Alt",
  Option: "Alt",
  Shift: "Shift",
  Return: "Enter",
  Enter: "Enter",
  Backspace: "Backspace",
  Delete: "Del",
  Escape: "Esc",
  Space: "Space",
  Tab: "Tab",
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
};

/**
 * Split on `+` rather than per character, so `F1` stays one keycap. A11Y-13:
 * display only — the accessible name uses the raw accelerator, which screen
 * readers handle far better than `⌘⇧V`.
 */
export function acceleratorGlyphs(
  accelerator: string,
  windows = isWindowsPlatform(),
): string[] {
  const glyphs = windows ? WINDOWS_GLYPHS : MAC_GLYPHS;
  return accelerator.split("+").map((token) => glyphs[token] ?? token);
}
