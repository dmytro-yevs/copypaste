import { CloudSyncSettings } from "@/features/settings/patterns/CloudSyncSettings";
import { DeviceSyncSettings } from "@/features/settings/patterns/DeviceSyncSettings";
import styles from "./SyncTab.module.css";

export function SyncTab() {
  return (
    <div className={styles.root}>
      <DeviceSyncSettings />
      <CloudSyncSettings />
    </div>
  );
}
