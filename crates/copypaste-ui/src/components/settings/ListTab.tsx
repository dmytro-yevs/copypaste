/**
 * The preview-lines slider is the one setting that changes layout: every row's
 * reserved height is a function of it (INV-5), so lowering it shrinks total
 * content height, which is exactly the case INV-6's clamp exists for (AT-4).
 * The value is formatted with its unit — a bare number told the user nothing
 * (CopyPaste-8ebg.63).
 */
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { useTranslation } from "@/i18n";
import { MAX_PREVIEW_LINES, MIN_PREVIEW_LINES } from "@/lib/layout";
import {
  HISTORY_DISPLAY_LIMITS,
  UNLIMITED_HISTORY_DISPLAY,
  usePrefs,
} from "@/store/prefs";
import { Row } from "@/components/settings/Row";

export function ListTab() {
  const { t } = useTranslation();
  const previewLines = usePrefs((s) => s.previewLines);
  const previewLinesPopup = usePrefs((s) => s.previewLinesPopup);
  const sortByDevice = usePrefs((s) => s.sortByDevice);
  const historyDisplayLimit = usePrefs((s) => s.historyDisplayLimit);
  const warnBeforeReveal = usePrefs((s) => s.warnBeforeReveal);
  const allowScreenshots = usePrefs((s) => s.allowScreenshots);
  const set = usePrefs((s) => s.set);

  return (
    <div className="flex flex-col">
      <Row
        title={t("settings.list.previewLines.title")}
        description={t("settings.list.previewLines.description")}
      >
        <div className="flex w-[220px] items-center gap-s-3">
          <Slider
            aria-label={t("settings.list.previewLines.title")}
            value={[previewLines]}
            min={MIN_PREVIEW_LINES}
            max={MAX_PREVIEW_LINES}
            step={1}
            onValueChange={([value]) =>
              value !== undefined && set("previewLines", value)
            }
          />
          <span className="w-14 shrink-0 text-right text-sm tabular-nums text-muted-foreground">
            {t("settings.list.previewLines.value", { count: previewLines })}
          </span>
        </div>
      </Row>

      <Row
        title={t("settings.list.groupByDevice.title")}
        description={t("settings.list.groupByDevice.description")}
      >
        <div className="flex items-center gap-s-2">
          <Switch
            id="group-by-device"
            aria-label={t("settings.list.groupByDevice.title")}
            checked={sortByDevice}
            onCheckedChange={(value) => set("sortByDevice", value)}
          />
          <Label htmlFor="group-by-device">
            {t(sortByDevice ? "common.on" : "common.off")}
          </Label>
        </div>
      </Row>

      <Row
        title={t("settings.list.popupPreviewLines.title")}
        description={t("settings.list.popupPreviewLines.description")}
      >
        <div className="flex w-[220px] items-center gap-s-3">
          <Slider
            aria-label={t("settings.list.popupPreviewLines.title")}
            value={[previewLinesPopup]}
            min={MIN_PREVIEW_LINES}
            max={MAX_PREVIEW_LINES}
            step={1}
            onValueChange={([value]) =>
              value !== undefined && set("previewLinesPopup", value)
            }
          />
          <span className="w-14 shrink-0 text-right text-sm tabular-nums text-muted-foreground">
            {t("settings.list.popupPreviewLines.value", {
              count: previewLinesPopup,
            })}
          </span>
        </div>
      </Row>

      <Row
        title={t("settings.list.historyDisplayLimit.title")}
        description={t("settings.list.historyDisplayLimit.description")}
      >
        <div className="flex w-[220px] items-center gap-s-3">
          <Slider
            aria-label={t("settings.list.historyDisplayLimit.title")}
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
          <span className="w-20 shrink-0 text-right text-sm tabular-nums text-muted-foreground">
            {historyDisplayLimit === UNLIMITED_HISTORY_DISPLAY
              ? t("settings.list.historyDisplayLimit.unlimited")
              : historyDisplayLimit.toLocaleString()}
          </span>
        </div>
      </Row>

      <Row
        title={t("settings.list.warnBeforeReveal.title")}
        description={t("settings.list.warnBeforeReveal.description")}
      >
        <div className="flex items-center gap-s-2">
          {/* A11Y-9: the visible label reads "On"/"Off", which names the state
              and not the setting, so the accessible name has to come from here. */}
          <Switch
            id="warn-before-reveal"
            aria-label={t("settings.list.warnBeforeReveal.title")}
            checked={warnBeforeReveal}
            onCheckedChange={(value) => set("warnBeforeReveal", value)}
          />
          <Label htmlFor="warn-before-reveal">
            {t(warnBeforeReveal ? "common.on" : "common.off")}
          </Label>
        </div>
      </Row>

      <Row
        title={t("settings.list.allowScreenshots.title")}
        description={t("settings.list.allowScreenshots.description")}
        note={
          allowScreenshots ? (
            <span className="text-xs text-warn-strong">
              {t("settings.list.allowScreenshots.warning")}
            </span>
          ) : undefined
        }
      >
        <div className="flex items-center gap-s-2">
          <Switch
            id="allow-screenshots"
            aria-label={t("settings.list.allowScreenshots.title")}
            checked={allowScreenshots}
            onCheckedChange={(value) => set("allowScreenshots", value)}
          />
          <Label htmlFor="allow-screenshots">
            {t(allowScreenshots ? "common.on" : "common.off")}
          </Label>
        </div>
      </Row>
    </div>
  );
}
