/** ⌘1–⌘9 only; there is no ⌘0 and no second row of ten. */
const QUICK_SLOTS = 9;

/** Search results renumber positions between keystrokes, so slots are disabled. */
export function quickSlot(key: string, searching: boolean): number | null {
  if (searching) return null;
  const digit = Number.parseInt(key, 10);
  if (!Number.isInteger(digit) || digit < 1 || digit > QUICK_SLOTS) return null;
  return digit - 1;
}
