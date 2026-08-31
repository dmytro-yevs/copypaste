import { Icon } from "@/components/ui";
import { clipTypeMetadata } from "@/lib/clipPresentation";
import type { Kind } from "@/lib/format";
import styles from "./ClipTypeGlyph.module.css";

export function ClipTypeGlyph({ kind, content }: { kind: Kind; content: string }) {
  const type = clipTypeMetadata(kind, content);
  return <span className={styles.root} data-kind={kind} title={type.label} aria-label={type.label}><Icon name={type.icon} size="xs" /></span>;
}
