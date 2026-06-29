// ShortcutCapture.tsx
// Extracted from SettingsView.tsx (CopyPaste-g06m.14 split) — cut/paste only.
import { useCallback, useRef, useState } from "react";

/** Convert a KeyboardEvent into a Tauri accelerator string like "CmdOrCtrl+Shift+V". */
export function eventToAccelerator(e: React.KeyboardEvent<HTMLInputElement>): string | null {
  // Ignore bare modifier keydowns (nothing to bind yet).
  if (["Meta", "Control", "Alt", "Shift"].includes(e.key)) return null;

  const parts: string[] = [];
  // On macOS Cmd maps to Meta; on other platforms Ctrl maps to CmdOrCtrl.
  // Tauri accepts "CmdOrCtrl" as a cross-platform alias.
  if (e.metaKey || e.ctrlKey) parts.push("CmdOrCtrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");

  // Always derive from the PHYSICAL key (e.code), not e.key, so the shortcut
  // is keyboard-layout-independent (e.g. Cyrillic layouts still record "Q").
  let key: string;
  if (e.code.startsWith("Key")) {
    key = e.code.slice(3); // "KeyQ" → "Q"
  } else if (e.code.startsWith("Digit")) {
    key = e.code.slice(5); // "Digit1" → "1"
  } else {
    key = e.code || e.key;
  }

  if (key.length === 1) {
    key = key.toUpperCase();
  } else {
    const keyMap: Record<string, string> = {
      ArrowUp: "Up",
      ArrowDown: "Down",
      ArrowLeft: "Left",
      ArrowRight: "Right",
      " ": "Space",
      Space: "Space",
      Escape: "Escape",
      Enter: "Return",
      Return: "Return",
      Backspace: "Backspace",
      Delete: "Delete",
      Tab: "Tab",
      Home: "Home",
      End: "End",
      PageUp: "PageUp",
      PageDown: "PageDown",
      F1: "F1",
      F2: "F2",
      F3: "F3",
      F4: "F4",
      F5: "F5",
      F6: "F6",
      F7: "F7",
      F8: "F8",
      F9: "F9",
      F10: "F10",
      F11: "F11",
      F12: "F12",
    };
    key = keyMap[key] ?? key;
  }
  // Require at least one modifier for a meaningful global shortcut.
  if (parts.length === 0) return null;

  parts.push(key);
  return parts.join("+");
}

/**
 * Render a Tauri accelerator string ("CmdOrCtrl+Shift+V") as Mac keycap symbols
 * ("⌘⇧V"). Modifiers collapse to their glyphs (no "+" separators) to match the
 * native macOS shortcut display. (audit P2)
 */
export function formatAccelerator(accel: string): string {
  const SYMBOL: Record<string, string> = {
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
  return accel
    .split("+")
    .map((part) => SYMBOL[part] ?? part)
    .join("");
}

export function ShortcutCapture({
  value,
  onChange,
}: {
  value: string;
  onChange: (accel: string) => void;
}) {
  const [capturing, setCapturing] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setCapturing(false);
        inputRef.current?.blur();
        return;
      }
      const accel = eventToAccelerator(e);
      if (accel !== null) {
        onChange(accel);
        setCapturing(false);
        inputRef.current?.blur();
      }
    },
    [onChange]
  );

  return (
    <input
      ref={inputRef}
      readOnly
      // audit P2: show Mac keycaps (⌘⇧V) instead of the raw "CmdOrCtrl+Shift+V"
      // accelerator token. The bound value is still the accelerator string.
      value={capturing ? "Press a shortcut…" : formatAccelerator(value)}
      onFocus={() => setCapturing(true)}
      onBlur={() => setCapturing(false)}
      onKeyDown={handleKeyDown}
      className={[
        "w-48 cursor-pointer border px-2.5 py-1.5 text-[13px] text-ide-text",
        // audit P2: bg-ide-bg looked disabled; use the white/elevated control fill.
        "outline-none select-none bg-ide-elevated",
        capturing
          ? "border-ide-accent ring-1 ring-ide-accent"
          : "border-ide-border hover:border-ide-accent",
      ].join(" ")}
      style={{ borderRadius: "var(--r-ctl)" }}
      title="Click and press a key combination"
    />
  );
}
