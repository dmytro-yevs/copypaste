import { Icon } from "@/components/ui/icon";
import { useId, useState } from "react";

import { SettingsRow } from "@/components/shared";
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
  const clearDescriptionId = useId();

  return (
    <>
      <Section title={t("settings.storage.historySection")}>
        <SettingsRow
          title={t("settings.storage.stored.title")}
          description={t("settings.storage.stored.description")}
        >
          <span className={styles.metric}>
            {status.isPending
              ? "Checking…"
              : status.isError || status.data === undefined
                ? "Unavailable"
                : status.data.toLocaleString()}
          </span>
        </SettingsRow>
      </Section>

      <Section title={t("settings.storage.dangerSection")}>
        <SettingsRow
          title={t("settings.storage.clear.title")}
          descriptionId={clearDescriptionId}
          description={t("settings.storage.clear.description")}
        >
          <Button
            variant="secondary"
            size="sm"
            tone="danger"
            disabled={clearStarting || pendingAll}
            aria-busy={clearStarting || pendingAll || undefined}
            aria-describedby={clearDescriptionId}
            onClick={() => setClearOpen(true)}
          >
            <Icon name="trash" aria-hidden="true" />
            {clearStarting || pendingAll ? "Clearing…" : t("settings.storage.clear.action")}
          </Button>
        </SettingsRow>
      </Section>

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
