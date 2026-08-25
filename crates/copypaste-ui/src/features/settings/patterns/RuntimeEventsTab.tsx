import { RuntimeLogViewer } from "@/features/diagnostics";
import styles from "./RuntimeEventsTab.module.css";

export function RuntimeEventsTab() {
  return (
    <div className={styles.root}>
      <RuntimeLogViewer />
    </div>
  );
}
