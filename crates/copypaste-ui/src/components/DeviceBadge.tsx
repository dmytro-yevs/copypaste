/**
 * DeviceBadge — shared origin-device chip (CopyPaste-bdac.31).
 *
 * Extracted from HistoryView.tsx:207-232. Shows a tinted bordered rounded
 * chip labelled "This device", a device name, or a compact UUID prefix.
 * The accent variant is used for the own device; the faint/dim variant for
 * remote devices. Returns null when originId is absent or empty so callers
 * can render unconditionally.
 */

/**
 * Return a compact label for an origin device.
 * - Empty / unknown → null (badge not shown)
 * - Matches own device → "This device"
 * - Known device name → that name (e.g. "MacBook Pro")
 * - Otherwise → first 8 chars of UUID as a fallback
 */
export function deviceLabel(
  originId: string | undefined,
  ownId: string,
  originName?: string | null
): string | null {
  if (!originId) return null;
  if (originId === "") return null;
  if (ownId && originId === ownId) return "This device";
  // Prefer the human-readable name from the daemon's devices table.
  if (originName) return originName;
  // Fallback: compact UUID prefix (first 8 chars is enough to distinguish devices).
  return originId.slice(0, 8);
}

export interface DeviceBadgeProps {
  originId: string | undefined;
  ownId: string;
  originName?: string | null;
}

/**
 * Compact chip indicating which device captured a clipboard entry.
 * Accent-tinted for the own device; dim/faint for remote devices.
 */
export function DeviceBadge({ originId, ownId, originName }: DeviceBadgeProps) {
  const label = deviceLabel(originId, ownId, originName);
  if (!label) return null;
  return (
    <span title={originId}>
      {label}
    </span>
  );
}
