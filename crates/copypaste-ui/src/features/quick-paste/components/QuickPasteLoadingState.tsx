import styles from "./QuickPasteLoadingState.module.css";

interface QuickPasteLoadingStateProps {
  title: string;
}

const rows = [
  "wide",
  "medium",
  "wide",
  "short",
  "medium",
  "wide",
  "short",
  "medium",
  "wide",
  "medium",
  "short",
  "wide",
  "medium",
  "wide",
  "short",
  "medium",
] as const;

export function QuickPasteLoadingState({ title }: QuickPasteLoadingStateProps) {
  return (
    <section
      className={styles.root}
      role="status"
      aria-label={title}
      aria-live="polite"
      aria-busy="true"
    >
      <div className={styles.rows} aria-hidden="true">
        {rows.map((width, index) => (
          <div
            className={styles.row}
            data-width={width}
            key={`${width}-${index}`}
          >
            <span className={styles.glyph} />
            <span className={styles.copy}>
              <span className={styles.line} />
              <span className={styles.meta}>
                <span className={styles.metaCopy} />
                <span className={styles.device} />
              </span>
            </span>
            <span className={styles.action} />
          </div>
        ))}
      </div>
    </section>
  );
}
