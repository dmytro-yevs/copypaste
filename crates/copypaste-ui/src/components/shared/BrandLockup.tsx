import { BrandMark } from "./BrandMark";
import styles from "./BrandLockup.module.css";

export function BrandLockup({ compact = false }: { compact?: boolean }) {
  return (
    <div className={styles.root} data-compact={compact || undefined}>
      <BrandMark size="sidebar" />
      <span className={styles.copy}>
        <strong>CopyPaste</strong>
        <small>Memory Stream</small>
      </span>
    </div>
  );
}
