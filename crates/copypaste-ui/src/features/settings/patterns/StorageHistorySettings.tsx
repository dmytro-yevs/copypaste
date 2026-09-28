import { useState } from "react";

import {
  AlertDialog,
} from "@/components/ui";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { settingsGroups } from "@/features/settings/model/settingsProjection";
import type { SettingsField } from "@/features/settings/model/settingsFieldSchema";
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
  const clearing = clearStarting || pendingAll;
  const fields: SettingsField[] = [
    status.isPending
      ? { kind: "status", definition: settingDefinition("storage", "settings.storage.stored.title"), mode: "loading", value: "Checking…" }
      : status.isError || status.data === undefined
        ? { kind: "status", definition: settingDefinition("storage", "settings.storage.stored.title"), mode: "error", value: "Unavailable" }
        : { kind: "readonly", definition: settingDefinition("storage", "settings.storage.stored.title"), value: <span className={styles.metric}>{status.data.toLocaleString()}</span> },
    { kind: "action", definition: settingDefinition("storage", "settings.storage.clear.title"),
      label: t("settings.storage.clear.action"), tone: "danger", icon: "trash", disabled: clearing,
      busy: clearing,
      onAction: () => setClearOpen(true) },
  ];

  return (
    <>
      <SettingsSchemaRenderer groups={settingsGroups("storage", fields, (key) => t(key as never))} />

      <AlertDialog
        open={clearOpen}
        onOpenChange={(open) => {
          if (!open && clearStarting) return;
          setClearOpen(open);
        }}
        title={t("history.clear.title")}
        description={t("history.clear.body")}
        cancel={{ label: t("common.cancel"), disabled: clearStarting }}
        action={{ label: t("history.clear.action"), tone: "danger", pending: clearStarting, onClick: async () => {
          setClearStarting(true);
          await removeAll();
          setClearStarting(false);
          setClearOpen(false);
        } }}
      />
    </>
  );
}
