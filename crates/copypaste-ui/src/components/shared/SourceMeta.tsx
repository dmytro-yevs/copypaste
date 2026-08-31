import type { ReactNode } from "react";

import { absoluteTime, shortAge, type Kind } from "@/lib/format";
import type { ClipSourceMetadata } from "@/lib/clipSourcePresentation";
import { originName, type OriginDevice } from "@/lib/itemOrigin";
import { cn } from "@/lib/cn";
import { DeviceMeta } from "./DeviceMeta";
import { ClipTypeGlyph } from "./ClipTypeGlyph";
import styles from "./SourceMeta.module.css";

export function SourceMeta({ source, sourceIcon, createdAt, origin, kind, content, extras, density = "regular", devicePresentation = "label" }: {
  source: ClipSourceMetadata;
  sourceIcon?: ReactNode;
  createdAt: number;
  origin: OriginDevice | null;
  kind: Kind;
  content: string;
  extras?: ReactNode;
  density?: "compact" | "regular";
  devicePresentation?: "label" | "icon";
}) {
  return <span className={cn(styles.root, styles[density])}><span className={styles.layout}>
    <ClipTypeGlyph kind={kind} content={content} />
    {source.available ? <span className={styles.app} title={source.label}>{sourceIcon ? <span className={styles.sourceIcon}>{sourceIcon}</span> : null}<span className={styles.appLabel}>{source.label}</span></span> : null}
    <span className={styles.unit}>{source.available ? <span aria-hidden="true" className={styles.separator}>•</span> : null}<span className={styles.age} title={absoluteTime(createdAt)}>{shortAge(createdAt)}</span></span>
    {extras}
    {origin ? <span className={styles.unit} data-device-kind={origin.kind} data-device-presentation={devicePresentation}><span aria-hidden="true" className={styles.separator}>•</span><DeviceMeta className={styles.device} label={originName(origin)} kind={origin.kind} iconOnly={devicePresentation === "icon"} /></span> : null}
  </span></span>;
}
