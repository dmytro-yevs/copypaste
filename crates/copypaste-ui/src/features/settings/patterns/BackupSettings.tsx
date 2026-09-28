import { useId, useState } from "react";

import { StateView } from "@/components/shared/StateView";
import {
  AlertDialog,
} from "@/components/ui";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { useBackupDatabase, useRestoreDatabase } from "@/hooks/useServiceConfig";
import { useTranslation } from "@/i18n";
import styles from "./StorageTab.module.css";

export function BackupSettings() {
  const { t } = useTranslation();
  const backup = useBackupDatabase();
  const restore = useRestoreDatabase();
  const [restoreOpen, setRestoreOpen] = useState(false);
  const backupFeedbackId = useId();
  const restoreFeedbackId = useId();

  return (
    <>
      <SettingsSchemaRenderer groups={[{ id: "recovery", title: t("settings.transfer.recoverySection"), fields: [{
          kind: "action", definition: settingDefinition("storage", "settings.transfer.backup.title"),
          note: backup.isError ? (
            <span id={backupFeedbackId}>
              <StateView mode="error" placement="control" title="History wasn’t backed up." />
            </span>
          ) : undefined,
          label: backup.isPending ? "Backing up…" : t("settings.transfer.backup.action"),
          icon: "file", disabled: backup.isPending, busy: backup.isPending,
          onAction: () => backup.mutate(),
        }, {
          kind: "action", definition: settingDefinition("storage", "settings.transfer.restore.title"),
          note: restore.isError ? (
            <span id={restoreFeedbackId}><StateView mode="error" placement="control" title="History wasn’t restored." /></span>
          ) : undefined,
          label: restore.isPending ? "Restoring…" : t("settings.transfer.restore.action"),
          icon: "reset", tone: "danger", disabled: restore.isPending, busy: restore.isPending,
          onAction: () => setRestoreOpen(true),
        }] }]} />


      <AlertDialog
        open={restoreOpen}
        onOpenChange={(open) => {
          if (!open && restore.isPending) return;
          setRestoreOpen(open);
        }}
        title={t("settings.transfer.restore.dialogTitle")}
        description={t("settings.transfer.restore.dialogBody")}
        cancel={{ label: t("common.cancel"), disabled: restore.isPending }}
        action={{ label: restore.isPending ? "Restoring…" : t("settings.transfer.restore.confirm"), tone: "danger", pending: restore.isPending, onClick: () => restore.mutate(undefined, { onSuccess: () => setRestoreOpen(false) }) }}
      ><p className={styles.hint}>{t("settings.transfer.restore.dialogSafety")}</p></AlertDialog>
    </>
  );
}
