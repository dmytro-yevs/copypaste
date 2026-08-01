/** Abnormal capture states stay visible beside History; healthy capture is
 * surfaced only in contextual Service and Diagnostics views. */
import { Button } from "@/components/ui/button";
import { useCaptureState } from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { TONE_DOT, toneOf } from "@/lib/capture";
import { cn } from "@/lib/cn";
import { useUi } from "@/store/ui";

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

  return (
    <div className="chrome flex shrink-0 flex-wrap items-center gap-s-2 border-b border-divider px-s-3 py-s-1">
      <div
        role="status"
        aria-label={t("capture.status.label", { summary })}
        title={summary}
        className="flex min-w-0 flex-1 items-center gap-s-2 text-xs text-muted-foreground"
      >
        <span
          aria-hidden="true"
          className={cn(
            "size-[var(--sz-badge-dot)] shrink-0 rounded-full",
            TONE_DOT[toneOf(snapshot.health)],
          )}
        />
        <span className="truncate">{snapshot.headline}</span>
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
  );
}
