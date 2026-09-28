import { useId, useState } from "react";

import { StateView } from "@/components/shared/StateView";
import {
  AlertDialog,
  Checkbox,
  Label,
} from "@/components/ui";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
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
  const exportFeedbackId = useId();
  const importFeedbackId = useId();

  return (
    <>
      <SettingsSchemaRenderer groups={[{ id: "transfer", title: t("settings.transfer.transferSection"), fields: [{
          kind: "action", definition: settingDefinition("storage", "settings.transfer.export.title"),
          note: exportHistory.isError ? (
            <span id={exportFeedbackId}>
              <StateView mode="error" placement="control" title="History wasn’t exported." />
            </span>
          ) : undefined,
          label: exportHistory.isPending ? "Exporting…" : t("settings.transfer.export.action"),
          icon: "download", disabled: exportHistory.isPending, busy: exportHistory.isPending,
          onAction: () => {
              // Sensitive-item consent is intentionally one export only.
              setIncludeSensitive(false);
              setExportOpen(true);
            },
        }, {
          kind: "action", definition: settingDefinition("storage", "settings.transfer.import.title"),
          note: importHistory.prepare.isError || importHistory.apply.isError ? (
            <span id={importFeedbackId}><StateView mode="error" placement="control" title="History wasn’t imported." /></span>
          ) : undefined,
          label: importHistory.isPending ? "Importing…" : t("settings.transfer.import.action"),
          icon: "upload", disabled: importHistory.isPending, busy: importHistory.isPending,
          onAction: () => importHistory.prepare.mutate(undefined, { onSuccess: (preview) => setPendingImport(preview) }),
        }] }]} />


      <AlertDialog
        open={exportOpen}
        onOpenChange={(open) => {
          if (!open && exportHistory.isPending) return;
          setExportOpen(open);
        }}
        title={t("settings.transfer.export.dialogTitle")}
        description={t("settings.transfer.export.dialogBody")}
        cancel={{ label: t("common.cancel"), disabled: exportHistory.isPending }}
        action={{ label: exportHistory.isPending ? "Exporting…" : t("settings.transfer.export.confirm"), pending: exportHistory.isPending, onClick: () => exportHistory.mutate(includeSensitive, { onSuccess: () => setExportOpen(false) }) }}
      >
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
      </AlertDialog>

      <AlertDialog
        open={pendingImport !== null}
        onOpenChange={(open) => {
          if (open || pendingImport === null || importHistory.apply.isPending) return;
          importHistory.cancel.mutate(pendingImport.token);
          setPendingImport(null);
        }}
        title={t("settings.transfer.import.dialogTitle", { count: pendingImport?.item_count ?? 0 })}
        description={t("settings.transfer.import.dialogBody", { count: pendingImport?.item_count ?? 0 })}
        cancel={{ label: t("common.cancel"), disabled: importHistory.apply.isPending }}
        action={{ label: importHistory.apply.isPending ? "Importing…" : t("settings.transfer.import.confirm"), pending: importHistory.apply.isPending, onClick: () => {
          if (pendingImport === null) return;
          importHistory.apply.mutate(pendingImport.token, { onSuccess: () => setPendingImport(null) });
        } }}
      />
    </>
  );
}
