/** Abnormal capture states stay visible beside History; healthy capture is
 * surfaced only in contextual Service and Diagnostics views. */
import { StateView, type StateMode } from "@/components/shared/StateView";
import { Button } from "@/components/ui";
import { capturePresentationOf } from "@/features/capture/model";
import { useCaptureState } from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";

export function CaptureStatus() {
  const { t } = useTranslation();
  const capture = useCaptureState();
  const openCaptureSettings = useUi((s) => s.openCaptureSettings);

  // A missing result and normal or intentionally disabled capture remain quiet.
  const snapshot = capture.data;
  if (snapshot === undefined) return null;
  const presentation = capturePresentationOf(snapshot.health);
  if (presentation.tone === "positive" || presentation.tone === "off") return null;

  const mode: StateMode = presentation.tone === "danger" ? "error"
    : presentation.tone === "attention" ? "warning" : "info";
  const summary = snapshot.detail
    ? `${snapshot.headline} ${snapshot.detail}`
    : snapshot.headline;

  return <StateView
    mode={mode}
    placement="inline"
    title={<span title={summary}>{snapshot.headline}</span>}
    aria-label={t("capture.status.label", { summary })}
    role={presentation.role}
    aria-live={presentation.urgency}
    actions={snapshot.rung === "desktop" ? undefined : (
      <Button
        variant="ghost"
        size="sm"
        title={t("capture.status.openHint")}
        aria-label={t("capture.status.open")}
        onClick={openCaptureSettings}
      >
        {t("capture.status.open")}
      </Button>
    )}
  />;
}
