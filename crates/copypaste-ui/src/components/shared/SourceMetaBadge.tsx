import type { ReactNode } from "react";

import { Icon, type IconName } from "@/components/ui";
import styles from "./SourceMetaBadge.module.css";

export function SourceMetaBadge({ icon, tone = "accent", label, title }: { icon: IconName; tone?: "accent" | "warning"; label: ReactNode; title?: string }) {
  const fullLabel = title ?? (typeof label === "string" ? label : undefined);
  return <span className={styles.unit} title={fullLabel}><span aria-hidden="true" className={styles.separator}>•</span><span className={`${styles.badge} ${styles[tone]}`} data-tone={tone}><Icon name={icon} size="xs" weight="bold" className={styles.icon} /><span className={styles.label}>{label}</span></span></span>;
}
