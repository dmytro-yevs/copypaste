// ShortcutCapture.tsx
// Extracted from SettingsView.tsx (CopyPaste-g06m.14 split) — cut/paste only.
import { useCallback, useRef, useState } from "react";

/** Convert a KeyboardEvent into a Tauri accelerator string like "CmdOrCtrl+Shift+V".
 *  Generic over HTMLElement (not HTMLInputElement) — only e.key/e.code/modifier
 *  flags are read, so it works from the div-based capture trigger below. */
export function eventToAccelerator(e: React.KeyboardEvent<HTMLElement>): string | null {
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

// Symbol table shared by formatAccelerator (joined string) and
// formatAcceleratorParts (one entry per modifier/key, used to render
// individual .kbd keycaps — task 5.6). Keyed on the raw Tauri accelerator
// token, e.g. "CmdOrCtrl" → "⌘". Unmapped tokens (F1…F12, letters, digits)
// pass through unchanged so a keycap still renders something sensible.
const ACCEL_SYMBOL: Record<string, string> = {
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
 * Split a Tauri accelerator string ("CmdOrCtrl+Shift+V") into one glyph/label
 * per modifier or key ("⌘", "⇧", "V") — each rendered as its own `.kbd`
 * keycap (task 5.6). Splitting on "+" (not per-character) keeps multi-char
 * unmapped keys like "F1" intact as a single keycap.
 */
export function formatAcceleratorParts(accel: string): string[] {
  if (!accel) return [];
  return accel.split("+").map((part) => ACCEL_SYMBOL[part] ?? part);
}

/**
 * Render a Tauri accelerator string ("CmdOrCtrl+Shift+V") as Mac keycap symbols
 * ("⌘⇧V"). Modifiers collapse to their glyphs (no "+" separators) to match the
 * native macOS shortcut display. (audit P2)
 */
export function formatAccelerator(accel: string): string {
  return formatAcceleratorParts(accel).join("");
}

export function ShortcutCapture({
  value,
  onChange,
}: {
  value: string;
  onChange: (accel: string) => void;
}) {
  const [capturing, setCapturing] = useState(false);
  // Focusable trigger — was an <input readOnly>; now a div so the captured
  // combo can render as a row of individual `.kbd` keycaps (task 5.6) instead
  // of one plain text value. Still focus/blur/keydown driven exactly as
  // before — only the element type changed, capture logic is untouched.
  const triggerRef = useRef<HTMLDivElement>(null);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setCapturing(false);
        triggerRef.current?.blur();
        return;
      }
      const accel = eventToAccelerator(e);
      if (accel !== null) {
        onChange(accel);
        setCapturing(false);
        triggerRef.current?.blur();
      }
    },
    [onChange]
  );

  // audit P2: show Mac keycaps (⌘ ⇧ V) instead of the raw "CmdOrCtrl+Shift+V"
  // accelerator token. The bound value is still the accelerator string —
  // this is display-only, split into one glyph per modifier/key.
  const parts = capturing ? ["Press a shortcut…"] : formatAcceleratorParts(value);

  return (
    <div
      ref={triggerRef}
      role="button"
      tabIndex={0}
      aria-label="Click and press a key combination"
      title="Click and press a key combination"
      onFocus={() => setCapturing(true)}
      onBlur={() => setCapturing(false)}
      onKeyDown={handleKeyDown}
      className="kbd-capture"
    >
      {parts.map((p, i) => (
        // parts is a short, order-stable, re-derived-each-render list — index
        // keys are fine (no reordering/insertion within the array).
        <span className="kbd" key={i}>{p}</span>
      ))}
    </div>
  );
}
