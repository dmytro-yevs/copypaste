/**
 * Capture state where the history is, not in a diagnostics screen — the android
 * doc's §5 rule 1, after v1 buried it and users found out that nothing had been
 * saved by going looking for something that was not there.
 *
 * The dot is decorative and the sentence carries the state, so the strip still
 * says everything it says with colour removed (A11Y-10). The sentence itself is
 * the snapshot's, verbatim.
 */
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

  // Nothing is rendered until something is known. A strip that guessed while
  // the answer was in flight would be the optimistic report CopyPaste-qzhu
  // removed, in a smaller costume.
  const snapshot = capture.data;
  if (snapshot === undefined) return null;

  const summary = snapshot.detail
    ? `${snapshot.headline} ${snapshot.detail}`
    : snapshot.headline;

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-s-2 border-b border-divider bg-panel px-s-3 py-s-1">
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
