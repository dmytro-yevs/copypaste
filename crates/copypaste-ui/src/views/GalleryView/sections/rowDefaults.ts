import { FIXTURE_OWN_DEVICE_ID } from "../../../lib/fixtures";

// ---------------------------------------------------------------------------
// Shared no-op prop bag for gallery-only <HistoryRow> examples (tasks 6.6/6.7/
// 6.11). HistoryRow takes ~15 props; every gallery section that renders one
// needs the same non-interactive defaults, so this is the single place that
// bag is assembled (mirrors the "shared source of truth" spirit of the
// fixtures factories — task 6.5 — but for prop defaults rather than data).
// ---------------------------------------------------------------------------

export const galleryRowDefaults = {
  selected: false,
  multiSelected: false,
  selectionMode: false,
  previewLines: 1,
  previewSize: 28,
  imageMaxHeight: 40,
  maskSensitive: true,
  showSensitiveWarnings: true,
  density: "comfortable" as const,
  ownDeviceId: FIXTURE_OWN_DEVICE_ID,
  onSelect: () => {},
  onToggleMultiSelect: () => {},
  onCopy: () => {},
  onPin: () => {},
  onDelete: () => {},
  onPreview: () => {},
};
