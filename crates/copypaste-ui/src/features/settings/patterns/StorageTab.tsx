import { BackupSettings } from "@/features/settings/patterns/BackupSettings";
import { StorageHistorySettings } from "@/features/settings/patterns/StorageHistorySettings";
import { TransferSettings } from "@/features/settings/patterns/TransferSettings";
import styles from "./StorageTab.module.css";

export function StorageTab() {
  return (
    <div className={styles.root}>
      <StorageHistorySettings />
      <TransferSettings />
      <BackupSettings />
    </div>
  );
}
