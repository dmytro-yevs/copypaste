import { Icon } from "@/components/ui/icon";
import { useId, useState } from "react";

import { FieldFeedback } from "@/components/shared";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Button,
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
              <FieldFeedback state="error">History wasn’t backed up.</FieldFeedback>
            </span>
          ) : undefined,
          label: backup.isPending ? "Backing up…" : t("settings.transfer.backup.action"),
          icon: "file", disabled: backup.isPending, busy: backup.isPending,
          onAction: () => backup.mutate(),
        }, {
          kind: "action", definition: settingDefinition("storage", "settings.transfer.restore.title"),
          note: restore.isError ? (
            <span id={restoreFeedbackId}><FieldFeedback state="error">History wasn’t restored.</FieldFeedback></span>
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
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("settings.transfer.restore.dialogTitle")}</AlertDialogTitle>
            <AlertDialogDescription>{t("settings.transfer.restore.dialogBody")}</AlertDialogDescription>
          </AlertDialogHeader>
          <p className={styles.hint}>{t("settings.transfer.restore.dialogSafety")}</p>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={restore.isPending}>
              <Icon name="close" aria-hidden="true" />
              {t("common.cancel")}
            </AlertDialogCancel>
            <Button
              tone="danger"
              disabled={restore.isPending}
              aria-busy={restore.isPending || undefined}
              onClick={() => restore.mutate(undefined, { onSuccess: () => setRestoreOpen(false) })}
            >
              <Icon name="reset" aria-hidden="true" />
              {restore.isPending ? "Restoring…" : t("settings.transfer.restore.confirm")}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
