import { Container, Screen } from "@/components/layout";
import { ScreenHeader } from "@/components/shared";
import styles from "./RouteLoadingState.module.css";

const rows = ["primary", "secondary", "primary", "short"] as const;

export function RouteLoadingState({
  title,
  label,
}: {
  title: string;
  label: string;
}) {
  return (
    <Screen
      className={styles.root}
      role="status"
      aria-label={label}
      aria-live="polite"
      aria-busy="true"
    >
      <Container width="library" gutter="screen">
        <ScreenHeader
          eyebrow={<span aria-hidden="true" className={`${styles.skeleton} ${styles.eyebrow}`} />}
          title={title}
          description={<span aria-hidden="true" className={`${styles.skeleton} ${styles.description}`} />}
        />
        <div className={styles.toolbar} aria-hidden="true">
          <span className={`${styles.skeleton} ${styles.search}`} />
          <span className={`${styles.skeleton} ${styles.control}`} />
          <span className={`${styles.skeleton} ${styles.control}`} />
        </div>
        <div className={styles.list} aria-hidden="true">
          {rows.map((width, index) => (
            <div className={styles.row} key={`${width}-${index}`}>
              <span className={`${styles.skeleton} ${styles.rowIcon}`} />
              <span className={styles.rowCopy} data-width={width}>
                <span className={`${styles.skeleton} ${styles.rowTitle}`} />
                <span className={`${styles.skeleton} ${styles.rowBody}`} />
              </span>
            </div>
          ))}
        </div>
      </Container>
    </Screen>
  );
}
