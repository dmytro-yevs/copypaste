import { type CSSProperties } from "react";
import type { HistoryEntry } from "../ipc";
import { sourceAppLabel } from "../ipc";
import { formatRelativeTime } from "../time";
import { deviceLabel } from "../../components/DeviceBadge";
import { normalizeContentKind } from "./normalizeContentKind";
import { KIND_PRESENTATION } from "./kindPresentation";

export interface ClipMetadataProps {
  entry: HistoryEntry;
  /** This device's id (to resolve the "This device" origin label). */
  ownDeviceId: string;
}

/**
 * Shared metadata line — `kind · sourceApp · relTime · originDevice` (design.md
 * Decision 8). The type-word is tinted with the kind's content-type token. The
 * source-app slot always renders when the daemon supplies an app name (generic
 * fallback, no per-app icon exists yet — Decision 8/C5).
 */
export function ClipMetadata({ entry, ownDeviceId }: ClipMetadataProps) {
  const kind = normalizeContentKind(entry);
  const p = KIND_PRESENTATION[kind];
  const app = sourceAppLabel(entry.app_bundle_id);
  const origin = deviceLabel(entry.origin_device_id, ownDeviceId, entry.origin_device_name);

  return (
    <div className="row__meta">
      <span className="ty" style={{ "--ct": `var(${p.token})` } as CSSProperties}>
        {p.label}
      </span>
      {` · ${formatRelativeTime(entry.wall_time, "short")}`}
      {app ? ` · ${app}` : ""}
      {origin ? ` · ${origin}` : ""}
    </div>
  );
}
