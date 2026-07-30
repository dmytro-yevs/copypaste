/**
 * The rung 2 screen: what is capturing, what is not, and the single next thing
 * to tap.
 *
 * Two shapes of this screen are load-bearing rather than cosmetic:
 *
 * * **A restart is not a failure.** Shizuku stops on every reboot, so
 *   "start it again" is a recurring, ordinary state; it gets `role="status"`,
 *   the attention tone and a one-tap action, never the alert role or the error
 *   tone. Only a refused read — the one thing nothing here can fix — is an
 *   alert.
 * * **No button is offered that cannot work.** CopyPaste can neither install
 *   Shizuku nor start it, so those steps offer a re-check and let the
 *   snapshot's own sentence say what the user has to do. `next_step` already
 *   refuses to name an action for a refused read, and this screen keeps that.
 *
 * Rungs 1 and 3 have no state in the model and no representation here.
 */
import { ClipboardPlus, TriangleAlert } from "lucide-react";

import { CaptureLadder } from "@/components/capture/CaptureLadder";
import { ToastNotice } from "@/components/capture/ToastNotice";
import { EmptyState } from "@/components/EmptyState";
import { Row } from "@/components/settings/Row";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  useCaptureMutation,
  useCaptureNow,
  useCaptureState,
} from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import {
  type CapturePrimary,
  TONE_DOT,
  ladderOf,
  primaryOf,
  toneOf,
} from "@/lib/capture";
import { cn } from "@/lib/cn";
import { longAge } from "@/lib/format";
import {
  type CaptureSnapshot,
  captureArm,
  captureRefresh,
  captureSetEnabled,
} from "@/lib/ipc";

const PRIMARY_LABEL = {
  arm: "capture.setup.action.arm",
  permission: "capture.setup.action.permission",
  recheck: "capture.setup.action.checkAgain",
} as const satisfies Record<Exclude<CapturePrimary, "none">, string>;

export function CaptureSetup() {
  const { t } = useTranslation();
  const capture = useCaptureState();
  const snapshot = capture.data;

  if (snapshot === undefined) {
    return capture.isPending ? (
      <EmptyState busy title={t("capture.loading.title")} body={t("capture.loading.body")} />
    ) : (
      <EmptyState
        icon={TriangleAlert}
        title={t("capture.unknown.title")}
        body={t("capture.unknown.body")}
        action={{ label: t("common.tryAgain"), onClick: () => void capture.refetch() }}
      />
    );
  }

  // macOS answers with the one state it has and refuses every mutation, so the
  // screen states it and offers nothing. This is the snapshot talking, not a
  // platform branch in the view.
  const managed = snapshot.rung !== "desktop";

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex shrink-0 items-center border-b border-divider bg-panel px-s-3 py-s-2">
        <h1 className="text-sm font-semibold">{t("capture.title")}</h1>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-s-3">
        <div className="mx-auto flex max-w-[var(--content-max-width)] flex-col gap-s-4">
          <StateCard snapshot={snapshot} />

          {snapshot.droppedClips > 0 && <Dropped count={snapshot.droppedClips} />}

          {managed && <AlwaysOn />}

          {managed && snapshot.shizuku.supported && (
            <>
              <CaptureLadder rungs={ladderOf(snapshot)} />
              <EnableRow enabled={snapshot.health.state !== "disabled"} />
            </>
          )}

          {managed && snapshot.shizuku.permission && (
            <ToastNotice suppressed={snapshot.toastSuppressed} />
          )}
        </div>
      </div>
    </div>
  );
}

function StateCard({ snapshot }: { snapshot: CaptureSnapshot }) {
  const { t } = useTranslation();
  const run = useCaptureMutation();

  const tone = toneOf(snapshot.health);
  const primary = primaryOf(snapshot.nextStep);

  return (
    <section
      // A11Y-5: only the state nothing can fix interrupts. A reboot asking for
      // one tap is information, and announcing it assertively every restart
      // would train the user to read a normal Tuesday as a fault.
      role={tone === "fault" ? "alert" : "status"}
      data-tone={tone}
      className="flex flex-col gap-s-3 rounded-lg border border-border bg-card p-s-3"
    >
      <div className="flex items-start gap-s-2">
        <span
          aria-hidden="true"
          className={cn(
            "mt-1 size-[var(--sz-badge-dot)] shrink-0 rounded-full",
            TONE_DOT[tone],
          )}
        />
        <div className="flex min-w-0 flex-col gap-s-1">
          <p className="text-sm font-medium">{snapshot.headline}</p>
          {snapshot.detail && (
            <p className="text-xs text-muted-foreground">{snapshot.detail}</p>
          )}
          {snapshot.lastCaptureAt !== null && (
            <p className="text-xs text-faint">
              {t("capture.setup.lastSaved", { age: longAge(snapshot.lastCaptureAt) })}
            </p>
          )}
        </div>
      </div>

      {primary !== "none" && (
        <div className="flex flex-wrap gap-s-2">
          <Button
            disabled={run.isPending}
            onClick={() =>
              run.mutate(
                primary === "recheck"
                  ? () => captureRefresh()
                  : () => captureArm(),
              )
            }
          >
            {t(run.isPending ? "capture.setup.action.busy" : PRIMARY_LABEL[primary])}
          </Button>
        </div>
      )}
    </section>
  );
}

/** Rung 0 has a doorway inside the app too, and it is the one surface that
 *  keeps working when everything below it lapses. */
function AlwaysOn() {
  const { t } = useTranslation();
  const now = useCaptureNow();

  return (
    <section className="flex flex-col gap-s-2">
      <h2 className="text-sm font-medium">{t("capture.setup.always.title")}</h2>
      <p className="text-xs text-muted-foreground">{t("capture.setup.always.body")}</p>
      <div className="flex flex-wrap gap-s-2">
        <Button
          variant="outline"
          disabled={now.isPending}
          onClick={() => now.mutate("in_app")}
        >
          <ClipboardPlus aria-hidden="true" />
          {t("capture.setup.always.action")}
        </Button>
      </div>
    </section>
  );
}

function EnableRow({ enabled }: { enabled: boolean }) {
  const { t } = useTranslation();
  const run = useCaptureMutation();

  return (
    <Row
      title={t("capture.setup.enable.title")}
      description={t("capture.setup.enable.body")}
    >
      <Switch
        checked={enabled}
        disabled={run.isPending}
        aria-label={t("capture.setup.enable.title")}
        onCheckedChange={(next) => run.mutate(() => captureSetEnabled(next))}
      />
    </Row>
  );
}

/** A copy that was taken and never stored. Stated, because the user finding a
 *  hole in their history for themselves is the outcome this feature exists to
 *  prevent. */
function Dropped({ count }: { count: number }) {
  const { t } = useTranslation();
  return (
    <p
      role="alert"
      className="flex items-center gap-s-2 rounded-md border border-warn/20 bg-warn/15 px-s-3 py-s-2 text-xs text-warn-strong"
    >
      <TriangleAlert size={14} aria-hidden="true" className="shrink-0" />
      {t("capture.setup.dropped", { count })}
    </p>
  );
}
