import { Icon } from "@/components/ui/icon";
import { useState } from "react";

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
import { useDeferredDelete } from "@/hooks/useDeferredDelete";
import { statusItemCount, useStatus } from "@/hooks/useStatus";
import { useTranslation } from "@/i18n";
import styles from "./StorageTab.module.css";

export function StorageHistorySettings() {
  const { t } = useTranslation();
  const status = useStatus(statusItemCount);
  // DMY-168: this page owns its deferred-delete instance; it never mounts with the history list.
  const { pendingAll, removeAll } = useDeferredDelete();
  const [clearOpen, setClearOpen] = useState(false);
  const [clearStarting, setClearStarting] = useState(false);

  return (
    <>
      <SettingsSchemaRenderer groups={[{ id: "history", title: t("settings.storage.historySection"), fields: [{
        kind: "readonly", definition: settingDefinition("storage", "settings.storage.stored.title"), value: <span className={styles.metric}>
            {status.isPending
              ? "Checking…"
              : status.isError || status.data === undefined
                ? "Unavailable"
                : status.data.toLocaleString()}
          </span>,
      }] }]} />

      <SettingsSchemaRenderer groups={[{ id: "danger", title: t("settings.storage.dangerSection"), fields: [{
        kind: "action", definition: settingDefinition("storage", "settings.storage.clear.title"),
        label: clearStarting || pendingAll ? "Clearing…" : t("settings.storage.clear.action"),
        tone: "danger", icon: "trash", disabled: clearStarting || pendingAll,
        busy: clearStarting || pendingAll, onAction: () => setClearOpen(true),
      }] }]} />

      <AlertDialog
        open={clearOpen}
        onOpenChange={(open) => {
          if (!open && clearStarting) return;
          setClearOpen(open);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("history.clear.title")}</AlertDialogTitle>
            <AlertDialogDescription>{t("history.clear.body")}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={clearStarting}>
              <Icon name="close" aria-hidden="true" />
              {t("common.cancel")}
            </AlertDialogCancel>
            <Button
              tone="danger"
              disabled={clearStarting}
              aria-busy={clearStarting || undefined}
              onClick={async () => {
                setClearStarting(true);
                await removeAll();
                setClearStarting(false);
                setClearOpen(false);
              }}
            >
              <Icon name="trash" aria-hidden="true" />
              {t("history.clear.action")}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
