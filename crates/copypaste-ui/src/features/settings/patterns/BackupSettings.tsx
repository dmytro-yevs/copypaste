import { Icon } from "@/components/ui/icon";
import { useId, useState } from "react";

import { FieldFeedback, SettingsRow } from "@/components/shared";
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
import { Section } from "@/features/settings/components/Section";
import { useBackupDatabase, useRestoreDatabase } from "@/hooks/useServiceConfig";
import { useTranslation } from "@/i18n";
import styles from "./StorageTab.module.css";

export function BackupSettings() {
  const { t } = useTranslation();
  const backup = useBackupDatabase();
  const restore = useRestoreDatabase();
  const [restoreOpen, setRestoreOpen] = useState(false);
  const backupDescriptionId = useId();
  const backupFeedbackId = useId();
  const restoreDescriptionId = useId();
  const restoreFeedbackId = useId();

  return (
    <>
      <Section title={t("settings.transfer.recoverySection")}>
        <SettingsRow
          title={t("settings.transfer.backup.title")}
          descriptionId={backupDescriptionId}
          description={t("settings.transfer.backup.description")}
          note={backup.isError ? (
            <span id={backupFeedbackId}>
              <FieldFeedback state="error">History wasn’t backed up.</FieldFeedback>
            </span>
          ) : undefined}
        >
          <Button
            variant="secondary"
            size="sm"
            disabled={backup.isPending}
            aria-busy={backup.isPending || undefined}
            aria-describedby={`${backupDescriptionId}${backup.isError ? ` ${backupFeedbackId}` : ""}`}
            onClick={() => backup.mutate()}
          >
            <Icon name="file" aria-hidden="true" />
            {backup.isPending ? "Backing up…" : t("settings.transfer.backup.action")}
          </Button>
        </SettingsRow>

        <SettingsRow
          title={t("settings.transfer.restore.title")}
          descriptionId={restoreDescriptionId}
          description={t("settings.transfer.restore.description")}
          note={restore.isError ? (
            <span id={restoreFeedbackId}>
              <FieldFeedback state="error">History wasn’t restored.</FieldFeedback>
            </span>
          ) : undefined}
        >
          <Button
            variant="secondary"
            size="sm"
            tone="danger"
            disabled={restore.isPending}
            aria-busy={restore.isPending || undefined}
            aria-describedby={`${restoreDescriptionId}${restore.isError ? ` ${restoreFeedbackId}` : ""}`}
            onClick={() => setRestoreOpen(true)}
          >
            <Icon name="reset" aria-hidden="true" />
            {restore.isPending ? "Restoring…" : t("settings.transfer.restore.action")}
          </Button>
        </SettingsRow>
      </Section>

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
