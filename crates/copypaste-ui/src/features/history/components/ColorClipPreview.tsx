import styles from "./ColorClipPreview.module.css";

export function ColorClipPreview({ value }: { value: string }) {
  const color = value.trim();
  return (
    <div className={styles.root}>
      <span
        aria-hidden="true"
        className={styles.swatch}
        style={{ backgroundColor: color }}
      />
      <strong>{color}</strong>
    </div>
  );
}
