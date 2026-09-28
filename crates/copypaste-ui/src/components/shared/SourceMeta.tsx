import type { ReactNode } from "react";

import { Icon, type IconName } from "@/components/ui";
import { clipTypeMetadata } from "@/lib/clipPresentation";
import { absoluteTime, shortAge, type Kind } from "@/lib/format";
import type { ClipSourceMetadata } from "@/lib/clipSourcePresentation";
import { originName, type OriginDevice } from "@/lib/itemOrigin";
import { cn } from "@/lib/cn";
import { DeviceMeta } from "./DeviceMeta";
import styles from "./SourceMeta.module.css";

export interface SourceMetaBadge {
  icon: IconName;
  tone?: "accent" | "warning";
  label: ReactNode;
  title?: string;
}

export function SourceMeta({ source, sourceIcon, createdAt, origin, kind, content, badges = [], density = "regular", devicePresentation = "label" }: {
  source: ClipSourceMetadata;
  sourceIcon?: ReactNode;
  createdAt: number;
  origin: OriginDevice | null;
  kind: Kind;
  content: string;
  badges?: readonly SourceMetaBadge[];
  density?: "compact" | "regular";
  devicePresentation?: "label" | "icon";
}) {
  const type = clipTypeMetadata(kind, content);
  return <span className={cn(styles.root, styles[density])}><span className={styles.layout}>
    <span className={styles.glyph} data-kind={kind} title={type.label} aria-label={type.label}><Icon name={type.icon} size="xs" /></span>
    {source.available ? <span className={styles.app} title={source.label}>{sourceIcon ? <span className={styles.sourceIcon}>{sourceIcon}</span> : null}<span className={styles.appLabel}>{source.label}</span></span> : null}
    <span className={styles.unit}>{source.available ? <span aria-hidden="true" className={styles.separator}>•</span> : null}<span className={styles.age} title={absoluteTime(createdAt)}>{shortAge(createdAt)}</span></span>
    {badges.map(({ icon, tone = "accent", label, title }, index) => (
      <span key={`${icon}-${index}`} className={styles.unit} title={title ?? (typeof label === "string" ? label : undefined)}>
        <span aria-hidden="true" className={styles.separator}>•</span>
        <span className={cn(styles.badge, styles[tone])} data-tone={tone}>
          <Icon name={icon} size="xs" weight="bold" className={styles.badgeIcon} />
          <span className={styles.badgeLabel}>{label}</span>
        </span>
      </span>
    ))}
    {origin ? <span className={styles.unit} data-device-kind={origin.kind} data-device-presentation={devicePresentation}><span aria-hidden="true" className={styles.separator}>•</span><DeviceMeta className={styles.device} label={originName(origin)} kind={origin.kind} iconOnly={devicePresentation === "icon"} /></span> : null}
  </span></span>;
}
