import type { ReactNode } from "react";

import { SourceAppIcon } from "@/features/source-apps";
import { ClipTypeGlyph } from "@/features/history/components/ClipTypeGlyph";
import { DeviceMeta } from "@/components/shared";
import { iconComponent } from "@/components/ui";
import type { ClipSourceMetadata } from "@/features/history/model/clipPresentation";
import type { OriginDevice } from "@/features/history/model/origin";
import { shortAge } from "@/lib/format";
import type { Kind } from "@/lib/format";
import type { Item } from "@/lib/ipc";
import { cn } from "@/lib/cn";
import styles from "./SourceMeta.module.css";

export function SourceMeta({
  item,
  source,
  origin,
  kind,
  content,
  extras,
  density = "regular",
  devicePresentation = "label",
}: {
  item: Item;
  source: ClipSourceMetadata;
  origin: OriginDevice | null;
  kind: Kind;
  content: string;
  extras?: ReactNode;
  density?: "compact" | "regular";
  devicePresentation?: "label" | "icon";
}) {
  const sourceAvailable = source.available;
  return (
     <span className={cn(styles.root, styles[density])}>
      <span className={styles.layout}>
        <ClipTypeGlyph kind={kind} content={content} />
        {sourceAvailable ? <span className={styles.app} title={source.label}>
            <SourceAppIcon
              bundleId={item.source_app_bundle_id}
              Fallback={iconComponent(source.icon)}
              fallbackText={source.label.slice(0, 2)}
              className={styles.sourceIcon}
            />
          <span className={styles.appLabel}>{source.label}</span>
        </span> : null}
        <span className={styles.unit}>
          {sourceAvailable ? <span aria-hidden="true" className={styles.separator}>•</span> : null}
          <span
            className={styles.age}
            title={new Date(item.created_at).toLocaleString()}
          >
            {shortAge(item.created_at)}
          </span>
        </span>
        {extras}
        {origin && (
          <span className={styles.unit} data-device-kind={origin.kind} data-device-presentation={devicePresentation}>
            <span aria-hidden="true" className={styles.separator}>•</span>
            <DeviceMeta
              className={styles.device}
              label={origin.name ?? origin.id.slice(0, 8)}
              kind={origin.kind}
              iconOnly={devicePresentation === "icon"}
            />
          </span>
        )}
      </span>
     </span>
  );
}
