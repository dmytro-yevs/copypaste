import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { useTranslation } from "@/i18n";
import {
  HISTORY_DISPLAY_LIMITS,
  UNLIMITED_HISTORY_DISPLAY,
  usePrefs,
} from "@/store/prefs";
import styles from "./ListTab.module.css";

export interface ListTabProps {
  ready: boolean;
  supportsScreenshots: boolean;
  scope?: "all" | "clipboard" | "privacy";
}

export function ListTab({ ready, supportsScreenshots, scope = "all" }: ListTabProps) {
  const { t } = useTranslation();
  const sortByDevice = usePrefs((s) => s.sortByDevice);
  const historyDisplayLimit = usePrefs((s) => s.historyDisplayLimit);
  const warnBeforeReveal = usePrefs((s) => s.warnBeforeReveal);
  const allowScreenshots = usePrefs((s) => s.allowScreenshots);
  const set = usePrefs((s) => s.set);
  if (!ready) return null;

  return <div className={styles.root}><SettingsSchemaRenderer groups={[
    ...(scope === "all" || scope === "clipboard" ? [{
      id: "history-list", title: "History list", fields: [
        { kind: "boolean" as const, definition: settingDefinition("clipboard", "settings.list.groupByDevice.title"), value: sortByDevice, controlId: "group-by-device", onChange: (value: boolean) => set("sortByDevice", value) },
        { kind: "number" as const, definition: settingDefinition("clipboard", "settings.list.historyDisplayLimit.title"), value: HISTORY_DISPLAY_LIMITS.indexOf(historyDisplayLimit), min: 0, max: HISTORY_DISPLAY_LIMITS.length - 1, displayValue: historyDisplayLimit === UNLIMITED_HISTORY_DISPLAY ? t("settings.list.historyDisplayLimit.unlimited") : historyDisplayLimit.toLocaleString(), onChange: (index: number) => { const next = HISTORY_DISPLAY_LIMITS[index]; if (next !== undefined) set("historyDisplayLimit", next); } },
      ],
    }] : []),
    ...(scope === "all" || scope === "privacy" ? [{
      id: "reveal-protection", title: "Reveal protection", fields: [
        { kind: "boolean" as const, definition: settingDefinition("privacy", "settings.list.warnBeforeReveal.title"), value: warnBeforeReveal, controlId: "warn-before-reveal", onChange: (value: boolean) => set("warnBeforeReveal", value) },
        { kind: "boolean" as const, definition: settingDefinition("privacy", "settings.list.allowScreenshots.title"), value: allowScreenshots, controlId: "allow-screenshots", visible: supportsScreenshots, onChange: (value: boolean) => set("allowScreenshots", value) },
      ],
    }] : []),
  ]} /></div>;
}
