/** Abnormal capture states stay visible beside History; healthy capture is
 * surfaced only in contextual Service and Diagnostics views. */
import { Button } from "@/components/ui";
import { toneOf } from "@/features/capture/model";
import { useCaptureState } from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";
import styles from "./CaptureStatus.module.css";

export function CaptureStatus() {
  const { t } = useTranslation();
  const capture = useCaptureState();
  const setView = useUi((s) => s.setView);

  // Never guess while the answer is in flight, and never show a static success
  // strip for normal capture.
  const snapshot = capture.data;
  if (snapshot === undefined || toneOf(snapshot.health) === "ok") return null;

  const summary = snapshot.detail
    ? `${snapshot.headline} ${snapshot.detail}`
    : snapshot.headline;
  const tone = toneOf(snapshot.health);

  return (
    <div className={styles.root}>
      <div className={styles.layout}>
        <div
          role="status"
          aria-label={t("capture.status.label", { summary })}
          title={summary}
          className={styles.status}
        >
          <span
            aria-hidden="true"
            className={styles.dot}
            data-tone={tone}
          />
          <span className={styles.headline}>{snapshot.headline}</span>
        </div>

        {/* macOS has no ladder and every mutating command there refuses, so the
            strip states the one thing it has to state and offers nothing. */}
        {snapshot.rung !== "desktop" && (
          <Button
            variant="ghost"
            size="sm"
            title={t("capture.status.openHint")}
            onClick={() => setView("capture")}
          >
            {t("capture.status.open")}
          </Button>
        )}
      </div>
    </div>
  );
}
