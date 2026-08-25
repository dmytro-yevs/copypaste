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
  Checkbox,
  Label,
} from "@/components/ui";
import { Section } from "@/features/settings/components/Section";
import { useExportHistory, useImportHistory } from "@/hooks/useServiceConfig";
import { useTranslation } from "@/i18n";
import type { ImportPreview } from "@/lib/ipc";
import styles from "./StorageTab.module.css";

export function TransferSettings() {
  const { t } = useTranslation();
  const exportHistory = useExportHistory();
  const importHistory = useImportHistory();
  const [exportOpen, setExportOpen] = useState(false);
  const [includeSensitive, setIncludeSensitive] = useState(false);
  const [pendingImport, setPendingImport] = useState<ImportPreview | null>(null);
  const exportDescriptionId = useId();
  const exportFeedbackId = useId();
  const importDescriptionId = useId();
  const importFeedbackId = useId();

  return (
    <>
      <Section title={t("settings.transfer.transferSection")}>
        <SettingsRow
          title={t("settings.transfer.export.title")}
          descriptionId={exportDescriptionId}
          description={t("settings.transfer.export.description")}
          note={exportHistory.isError ? (
            <span id={exportFeedbackId}>
              <FieldFeedback state="error">History wasn’t exported.</FieldFeedback>
            </span>
          ) : undefined}
        >
          <Button
            variant="secondary"
            size="sm"
            disabled={exportHistory.isPending}
            aria-busy={exportHistory.isPending || undefined}
            aria-describedby={`${exportDescriptionId}${exportHistory.isError ? ` ${exportFeedbackId}` : ""}`}
            onClick={() => {
              // Sensitive-item consent is intentionally one export only.
              setIncludeSensitive(false);
              setExportOpen(true);
            }}
          >
            <Icon name="download" aria-hidden="true" />
            {exportHistory.isPending ? "Exporting…" : t("settings.transfer.export.action")}
          </Button>
        </SettingsRow>

        <SettingsRow
          title={t("settings.transfer.import.title")}
          descriptionId={importDescriptionId}
          description={t("settings.transfer.import.description")}
          note={importHistory.prepare.isError || importHistory.apply.isError ? (
            <span id={importFeedbackId}>
              <FieldFeedback state="error">History wasn’t imported.</FieldFeedback>
            </span>
          ) : undefined}
        >
          <Button
            variant="secondary"
            size="sm"
            disabled={importHistory.isPending}
            aria-busy={importHistory.isPending || undefined}
            aria-describedby={`${importDescriptionId}${importHistory.prepare.isError || importHistory.apply.isError ? ` ${importFeedbackId}` : ""}`}
            onClick={() => {
              importHistory.prepare.mutate(undefined, {
                onSuccess: (preview) => setPendingImport(preview),
              });
            }}
          >
            <Icon name="upload" aria-hidden="true" />
            {importHistory.isPending ? "Importing…" : t("settings.transfer.import.action")}
          </Button>
        </SettingsRow>
      </Section>

      <AlertDialog
        open={exportOpen}
        onOpenChange={(open) => {
          if (!open && exportHistory.isPending) return;
          setExportOpen(open);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("settings.transfer.export.dialogTitle")}</AlertDialogTitle>
            <AlertDialogDescription>{t("settings.transfer.export.dialogBody")}</AlertDialogDescription>
          </AlertDialogHeader>
          <div className={styles.exportOptions}>
            <div className={styles.checkboxRow}>
              <Checkbox
                id="export-include-sensitive"
                checked={includeSensitive}
                onCheckedChange={(checked) => setIncludeSensitive(checked === true)}
              />
              <Label htmlFor="export-include-sensitive">
                {t("settings.transfer.export.includeSensitive")}
              </Label>
            </div>
            <p className={styles.hint}>{t("settings.transfer.export.includeSensitiveHint")}</p>
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={exportHistory.isPending}>
              <Icon name="close" aria-hidden="true" />
              {t("common.cancel")}
            </AlertDialogCancel>
            <Button
              disabled={exportHistory.isPending}
              aria-busy={exportHistory.isPending || undefined}
              onClick={() => exportHistory.mutate(includeSensitive, { onSuccess: () => setExportOpen(false) })}
            >
              <Icon name="download" aria-hidden="true" />
              {exportHistory.isPending ? "Exporting…" : t("settings.transfer.export.confirm")}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog
        open={pendingImport !== null}
        onOpenChange={(open) => {
          if (open || pendingImport === null || importHistory.apply.isPending) return;
          importHistory.cancel.mutate(pendingImport.token);
          setPendingImport(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("settings.transfer.import.dialogTitle", { count: pendingImport?.item_count ?? 0 })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("settings.transfer.import.dialogBody", { count: pendingImport?.item_count ?? 0 })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={importHistory.apply.isPending}>
              <Icon name="close" aria-hidden="true" />
              {t("common.cancel")}
            </AlertDialogCancel>
            <Button
              disabled={importHistory.apply.isPending}
              aria-busy={importHistory.apply.isPending || undefined}
              onClick={() => {
                if (pendingImport === null) return;
                importHistory.apply.mutate(pendingImport.token, {
                  onSuccess: () => setPendingImport(null),
                });
              }}
            >
              <Icon name="upload" aria-hidden="true" />
              {importHistory.apply.isPending ? "Importing…" : t("settings.transfer.import.confirm")}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
