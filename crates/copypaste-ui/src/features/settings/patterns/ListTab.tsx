import { useId } from "react";
import { Slider, Switch } from "@/components/ui";
import { useTranslation } from "@/i18n";
import {
  HISTORY_DISPLAY_LIMITS,
  UNLIMITED_HISTORY_DISPLAY,
  usePrefs,
} from "@/store/prefs";
import { SettingsRow } from "@/components/shared";
import { Section } from "@/features/settings/components/Section";
import styles from "./ListTab.module.css";

interface ListTabProps {
  ready: boolean;
  supportsScreenshots: boolean;
  scope?: "all" | "clipboard" | "privacy";
}

export function ClipboardListSettings(props: Omit<ListTabProps, "scope">) {
  return <ListTab {...props} scope="clipboard" />;
}

export function PrivacyDisplaySettings(props: Omit<ListTabProps, "scope">) {
  return <ListTab {...props} scope="privacy" />;
}

export function ListTab({
  ready,
  supportsScreenshots,
  scope = "all",
}: ListTabProps) {
  const { t } = useTranslation();
  const sortByDevice = usePrefs((s) => s.sortByDevice);
  const historyDisplayLimit = usePrefs((s) => s.historyDisplayLimit);
  const warnBeforeReveal = usePrefs((s) => s.warnBeforeReveal);
  const allowScreenshots = usePrefs((s) => s.allowScreenshots);
  const set = usePrefs((s) => s.set);
  const groupingDescriptionId = useId();
  const limitDescriptionId = useId();
  const revealDescriptionId = useId();
  const screenshotsDescriptionId = useId();

  if (!ready) {
    return null;
  }

  return (
    <div className={styles.root}>
      {(scope === "all" || scope === "clipboard") && <Section title="History list">
        <SettingsRow
          title={t("settings.list.groupByDevice.title")}
          descriptionId={groupingDescriptionId}
          description={t("settings.list.groupByDevice.description")}
        >
          <Switch
            id="group-by-device"
            aria-label={t("settings.list.groupByDevice.title")}
            aria-describedby={groupingDescriptionId}
            checked={sortByDevice}
            onCheckedChange={(value) => set("sortByDevice", value)}
          />
        </SettingsRow>

        <SettingsRow
          title={t("settings.list.historyDisplayLimit.title")}
          descriptionId={limitDescriptionId}
          description={t("settings.list.historyDisplayLimit.description")}
        >
          <div className={styles.sliderControl}>
            <output className={styles.wideSliderValue}>
              {historyDisplayLimit === UNLIMITED_HISTORY_DISPLAY
                ? t("settings.list.historyDisplayLimit.unlimited")
                : historyDisplayLimit.toLocaleString()}
            </output>
            <Slider
              aria-label={t("settings.list.historyDisplayLimit.title")}
              aria-describedby={limitDescriptionId}
              value={[HISTORY_DISPLAY_LIMITS.indexOf(historyDisplayLimit)]}
              min={0}
              max={HISTORY_DISPLAY_LIMITS.length - 1}
              step={1}
              onValueChange={([index]) => {
                const value =
                  index === undefined ? undefined : HISTORY_DISPLAY_LIMITS[index];
                if (value !== undefined) set("historyDisplayLimit", value);
              }}
            />
          </div>
        </SettingsRow>
      </Section>}

      {(scope === "all" || scope === "privacy") && <Section title="Reveal protection">
        <SettingsRow
          title={t("settings.list.warnBeforeReveal.title")}
          descriptionId={revealDescriptionId}
          description={t("settings.list.warnBeforeReveal.description")}
        >
          <Switch
            id="warn-before-reveal"
            aria-label={t("settings.list.warnBeforeReveal.title")}
            aria-describedby={revealDescriptionId}
            checked={warnBeforeReveal}
            onCheckedChange={(value) => set("warnBeforeReveal", value)}
          />
        </SettingsRow>

        {supportsScreenshots ? (
          <SettingsRow
            title={t("settings.list.allowScreenshots.title")}
            descriptionId={screenshotsDescriptionId}
            description={t("settings.list.allowScreenshots.description")}
            note={
              allowScreenshots ? (
                <span className={styles.warning}>
                  {t("settings.list.allowScreenshots.warning")}
                </span>
              ) : undefined
            }
          >
            <Switch
              id="allow-screenshots"
              aria-label={t("settings.list.allowScreenshots.title")}
              aria-describedby={screenshotsDescriptionId}
              checked={allowScreenshots}
              onCheckedChange={(value) => set("allowScreenshots", value)}
            />
          </SettingsRow>
        ) : null}
      </Section>}
    </div>
  );
}
