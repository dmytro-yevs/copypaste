import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";

import { Icon } from "@/components/ui/icon";

import {
  SettingsRow,
} from "@/components/shared";
import { Button } from "@/components/ui";
import { StateView, type StateMode } from "@/components/shared/StateView";
import { SettingsGroupSurface } from "@/features/settings/components/SettingsGroupSurface";
import {
  type CapturePrimary,
  primaryOf,
  capturePresentationOf,
} from "@/features/capture/model";
import {
  useCaptureMutation,
  useCaptureNow,
} from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { longAge } from "@/lib/format";
import {
  type CaptureSnapshot,
  captureArm,
  captureOpenDeveloperOptions,
  captureOpenShizuku,
  captureRefresh,
  captureSetupInstructions,
  copyText,
  type CaptureSetupInstructions,
} from "@/lib/ipc";
import { usePrefs } from "@/store/prefs";
import styles from "./CaptureSetup.module.css";
import { useCaptureSetupResolution } from "./useCaptureSetupResolution";

const PRIMARY_LABEL = {
  arm: "capture.setup.action.arm",
  permission: "capture.setup.action.permission",
  recheck: "capture.setup.action.checkAgain",
} as const satisfies Record<Exclude<CapturePrimary, "none">, string>;

export function CaptureSetupController() {
  const { t } = useTranslation();
  const resolved = useCaptureSetupResolution();

  if (resolved.kind === "ready") return <CaptureSetup snapshot={resolved.snapshot} />;

  return (
    <div className={styles.emptyState}>
      {resolved.kind === "loading" ? (
        <StateView mode="loading" placement="panel" title={t("capture.loading.title")} description={t("capture.loading.body")} />
      ) : (
        <StateView
          mode="error"
          placement="panel"
          title={t("capture.unknown.title")}
          description={t("capture.unknown.body")}
          actions={<Button onClick={resolved.retry}><Icon name="refresh" size="sm" />{t("common.tryAgain")}</Button>}
        />
      )}
    </div>
  );
}

export function CaptureSetup({ snapshot }: { snapshot: CaptureSnapshot }) {
  const { t } = useTranslation();
  const managed = snapshot.rung !== "desktop";

  return (
    <div className={styles.content}>
      <h2 className={styles.heading}>{t("capture.title")}</h2>
      <CaptureStateCard snapshot={snapshot} suppressAction={managed} />
      {snapshot.droppedClips > 0 && <Dropped count={snapshot.droppedClips} />}
      {managed ? <AndroidCaptureRecovery snapshot={snapshot} /> : null}
      <SettingsGroupSurface><AlwaysOn /></SettingsGroupSurface>
    </div>
  );
}

function CaptureStateCard({
  snapshot,
  suppressAction = false,
}: {
  snapshot: CaptureSnapshot;
  suppressAction?: boolean;
}) {
  const { t } = useTranslation();
  const run = useCaptureMutation();
  const presentation = capturePresentationOf(snapshot.health);
  const primary = primaryOf(snapshot.nextStep);

  const action = suppressAction || primary === "none" ? undefined : (
    <Button
      state={run.isPending ? "loading" : "normal"}
      onClick={() =>
        run.mutate(
          primary === "recheck"
            ? () => captureRefresh()
            : () => captureArm(),
        )
      }
    >
      <Icon name="refresh" size="md" aria-hidden="true" />
      {t(PRIMARY_LABEL[primary])}
    </Button>
  );

  const mode: StateMode = presentation.tone === "positive" ? "success"
    : presentation.tone === "danger" ? "error"
    : presentation.tone === "attention" ? "warning" : "info";
  const lastSaved = snapshot.lastCaptureAt === null ? undefined : t("capture.setup.lastSaved", {
    age: longAge(snapshot.lastCaptureAt),
  });

  return (
    <StateView
      mode={run.isPending ? "loading" : mode}
      placement="panel"
      title={snapshot.headline}
      description={<>{snapshot.detail}{lastSaved ? <small>{lastSaved}</small> : null}</>}
      actions={action}
      role={presentation.role}
      aria-live={presentation.urgency}
      aria-busy={run.isPending || undefined}
    />
  );
}

function AlwaysOn() {
  const { t } = useTranslation();
  const now = useCaptureNow();

  return (
    <SettingsRow
      title={t("capture.setup.always.title")}
      help={t("capture.setup.always.body")}
    >
      <Button
        variant="secondary"
        size="sm"
        state={now.isPending ? "loading" : "normal"}
        onClick={() => now.mutate("in_app")}
      >
        <Icon name="copy" size="sm" />
        {t("capture.setup.always.action")}
      </Button>
    </SettingsRow>
  );
}

function AndroidCaptureRecovery({ snapshot }: { snapshot: CaptureSnapshot }) {
  const { t } = useTranslation();
  const progress = usePrefs((state) => state.onboarding);
  const checkpointOnboarding = usePrefs((state) => state.checkpointOnboarding);
  const instructions = useQuery<CaptureSetupInstructions>({
    queryKey: ["capture", "setup-instructions"],
    queryFn: captureSetupInstructions,
    retry: false,
  });
  const refresh = useCaptureMutation();
  const arm = useCaptureMutation();
  const shizuku = useMutation({ mutationFn: captureOpenShizuku });
  const developerOptions = useMutation({ mutationFn: captureOpenDeveloperOptions });
  const [checkpointing, setCheckpointing] = useState(false);
  const [feedback, setFeedback] = useState<{
    state: "success" | "error";
    message: string;
  } | null>(null);
  const readyToArm = snapshot.nextStep === "arm";

  if (snapshot.health.state === "working") return null;

  const commands = progress.captureSetupMethod === "shizuku"
    ? instructions.data?.shizukuCommands
    : progress.captureSetupMethod === "adb"
      ? instructions.data?.adbCommands
      : undefined;

  const checkpoint = async (patch: Parameters<typeof checkpointOnboarding>[0]) => {
    setCheckpointing(true);
    const saved = await checkpointOnboarding(patch);
    setCheckpointing(false);
    if (!saved) {
      setFeedback({ state: "error", message: t("onboarding.capture.setup.saveFailed") });
    }
    return saved;
  };

  const choose = async (method: "shizuku" | "adb") => {
    if (await checkpoint({ captureSetupMethod: method, captureSetupStage: "commands" })) {
      setFeedback(null);
    }
  };

  const copy = async (argv: readonly string[]) => {
    if (!await checkpoint({ captureSetupStage: "verify" })) return;
    try {
      await copyText(formatCommand(argv));
      setFeedback({ state: "success", message: t("onboarding.capture.setup.copied") });
    } catch {
      setFeedback({ state: "error", message: t("onboarding.capture.setup.copyFailed") });
    }
  };

  const finishFromSnapshot = async (fresh: CaptureSnapshot) => {
    if (fresh.health.state === "working") {
      if (!await checkpoint({ captureSetupStage: "complete" })) return;
      setFeedback({ state: "success", message: t("onboarding.capture.setup.complete") });
      return;
    }
    setFeedback({ state: "error", message: t("onboarding.capture.setup.notReady") });
  };

  const applyShizuku = async () => {
    if (!await checkpoint({ captureSetupMethod: "shizuku", captureSetupStage: "verify" })) return;
    arm.mutate(() => captureArm(), { onSuccess: (fresh) => void finishFromSnapshot(fresh) });
  };

  return (
    <section className={styles.recovery} aria-labelledby="capture-recovery-title">
      <h3 id="capture-recovery-title">{t("onboarding.capture.setup.title")}</h3>
      <p>{t("onboarding.capture.setup.body")}</p>
      <div className={styles.methodChoices} role="radiogroup" aria-label={t("onboarding.capture.setup.title")}>
        <Button
          type="button"
          variant="secondary"
          role="radio"
          aria-checked={progress.captureSetupMethod === "shizuku"}
          disabled={checkpointing || arm.isPending}
          onClick={() => void choose("shizuku")}
        >
          {t("onboarding.capture.setup.shizuku")}
        </Button>
        <Button
          type="button"
          variant="secondary"
          role="radio"
          aria-checked={progress.captureSetupMethod === "adb"}
          disabled={checkpointing || arm.isPending}
          onClick={() => void choose("adb")}
        >
          {t("onboarding.capture.setup.adb")}
        </Button>
      </div>
      {progress.captureSetupMethod === "shizuku" ? (
        <div className={styles.recoveryActions}>
          <Button
            type="button"
            variant="secondary"
            disabled={checkpointing || shizuku.isPending}
            onClick={async () => {
              if (!await checkpoint({ captureSetupMethod: "shizuku", captureSetupStage: "commands" })) return;
              shizuku.mutate();
            }}
          >
            {t("onboarding.capture.setup.openShizuku")}
          </Button>
          <Button
            type="button"
            variant="ghost"
            disabled={developerOptions.isPending}
            onClick={() => developerOptions.mutate()}
          >
            {t("onboarding.capture.setup.openDeveloperOptions")}
          </Button>
          <Button
            type="button"
            disabled={checkpointing || arm.isPending}
            state={arm.isPending ? "loading" : "normal"}
            onClick={() => void applyShizuku()}
          >
            {t("onboarding.capture.setup.apply")}
          </Button>
        </div>
      ) : progress.captureSetupMethod === "adb" ? (
        <p>{t("onboarding.capture.setup.adbDetail")}</p>
      ) : null}
      {instructions.isError ? <StateView mode="error" placement="control" title={t("onboarding.capture.setup.unavailable")} /> : null}
      {progress.captureSetupMethod === "adb" && commands?.length ? (
        <div className={styles.commands}>
          <h4>{t("onboarding.capture.setup.commands")}</h4>
          {commands.map((argv: readonly string[], index: number) => (
            <div key={index} className={styles.command}>
              <code>{formatCommand(argv)}</code>
              <Button type="button" size="sm" variant="secondary" disabled={checkpointing} onClick={() => void copy(argv)}>
                {t("onboarding.capture.setup.copy")}
              </Button>
            </div>
          ))}
        </div>
      ) : null}
      {instructions.data?.requiresRestart ? <StateView mode="info" placement="control" title={t("onboarding.capture.setup.restart")} role="none" /> : null}
      {feedback ? <StateView mode={feedback.state} placement="control" title={feedback.message} /> : null}
      <div className={styles.recoveryActions}>
        <Button
          type="button"
          variant="secondary"
          state={refresh.isPending ? "loading" : "normal"}
          disabled={checkpointing || refresh.isPending}
          onClick={() => refresh.mutate(() => captureRefresh(), {
            onSuccess: (fresh) => void finishFromSnapshot(fresh),
          })}
        >
          {t(refresh.isPending ? "onboarding.capture.setup.verifying" : "onboarding.capture.setup.verify")}
        </Button>
        {readyToArm ? (
          <Button
            type="button"
            state={arm.isPending ? "loading" : "normal"}
            onClick={() => arm.mutate(() => captureArm(), {
              onSuccess: (fresh) => void finishFromSnapshot(fresh),
            })}
          >
            {t("onboarding.capture.setup.arm")}
          </Button>
        ) : null}
      </div>
    </section>
  );
}

function formatCommand(argv: readonly string[]): string {
  return argv.map((part) => /^[A-Za-z0-9_@%+=:,./-]+$/.test(part)
    ? part
    : `'${part.replace(/'/g, "'\\\"'\\\"'")}'`).join(" ");
}

function Dropped({ count }: { count: number }) {
  const { t } = useTranslation();
  return (
    <StateView mode="warning" placement="inline" role="alert" title={t("capture.setup.dropped", { count })} />
  );
}
