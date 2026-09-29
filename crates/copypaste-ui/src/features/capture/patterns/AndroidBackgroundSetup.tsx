import { useEffect, useState } from "react";
import { createPortal } from "react-dom";

import { StateView } from "@/components/shared/StateView";
import { Button, Stepper, Tabs } from "@/components/ui";
import { capturePresentationOf } from "@/features/capture/model";
import { useTranslation } from "@/i18n";
import { captureArm, captureOpenShizuku, captureRefresh, copyText } from "@/lib/ipc";
import { useAndroidCaptureSetup } from "./useAndroidCaptureSetup";
import styles from "./AndroidBackgroundSetup.module.css";

const SHIZUKU_STEPS = ["install", "start", "authorize", "verify"] as const;

export function AndroidBackgroundSetup({ onReadyChange, onBusyChange, actionContainer }: { onReadyChange?: (ready: boolean) => void; onBusyChange?: (busy: boolean) => void; actionContainer?: HTMLElement | null }) {
  const { t } = useTranslation();
  const { capture, method, instructions, action, error } = useAndroidCaptureSetup();
  const [copied, setCopied] = useState<number | null>(null);
  const snapshot = capture.data;
  const working = snapshot?.health.state === "working";
  const current = snapshot?.nextStep === "arm" || snapshot?.health.state === "granted_not_working" ? 3 : !snapshot?.shizuku.installed ? 0 : !snapshot.shizuku.running ? 1 : !snapshot.shizuku.permission ? 2 : 3;
  const step = SHIZUKU_STEPS[current];
  const busy = action.isPending;
  const done = [snapshot?.shizuku.installed, snapshot?.shizuku.running, snapshot?.shizuku.permission, working];
  useEffect(() => { onReadyChange?.(working); }, [working, onReadyChange]);
  useEffect(() => { onBusyChange?.(busy); }, [busy, onBusyChange]);

  if (!snapshot) return <StateView mode={capture.isError ? "error" : "loading"} placement="panel"
    title={t(capture.isError ? "capture.unknown.title" : "capture.loading.title")}
    actions={capture.isError ? <Button onClick={() => void capture.refetch()}>{t("common.tryAgain")}</Button> : undefined} />;

  if (working) return <StateView mode="success" placement="panel" title={t("onboarding.capture.setup.complete")} />;

  const presentation = capturePresentationOf(snapshot.health);
  const run = (operation: () => ReturnType<typeof captureArm> | Promise<void>, stage: "commands" | "verify" = "commands") => {
    setCopied(null);
    action.mutate({ run: operation, stage });
  };
  const needsArming = snapshot.nextStep === "arm" || snapshot.health.state !== "granted_not_working" || snapshot.health.reason === "not_armed";
  const copyCommand = (command: readonly string[], index: number) => {
    setCopied(null);
    action.mutate({ stage: "verify", run: () => copyText(formatCaptureCommand(command)) }, { onSuccess: () => setCopied(index) });
  };
  const shizuku = !snapshot.shizuku.supported && current < 3 ? (
    <StateView mode="info" placement="inline" title={t("onboarding.capture.setup.androidVersion")} />
  ) : (
    <div className={styles.guide}>
      <Stepper variant="compact" label={t("onboarding.capture.setup.shizukuSteps")} items={SHIZUKU_STEPS.map((id, index) => ({
        id, label: t(`onboarding.capture.setup.${id}.title`), icon: "circle", done: done[index], current: index === current,
        stateLabel: t(`onboarding.capture.setup.${done[index] ? "done" : index === current ? "current" : "next"}`),
      }))} />
      <div className={styles.stepCopy}>
        <p className={styles.note}>{t("onboarding.slideLabel", { current: current + 1, total: SHIZUKU_STEPS.length })}</p>
        <h2>{t(`onboarding.capture.setup.${step}.title`)}</h2>
        <p>{t(`onboarding.capture.setup.${step}.body`)}</p>
      </div>
      {step === "start" ? <ol className={styles.instructions}>
        <li>{t("onboarding.capture.setup.start.developer")}</li>
        <li>{t("onboarding.capture.setup.start.pair")}</li>
        <li>{t("onboarding.capture.setup.start.start")}</li>
      </ol> : null}
      {step === "install" || step === "start" ? <Button variant="ghost" disabled={busy} onClick={() => run(captureRefresh, "verify")}>{t("onboarding.capture.setup.check")}</Button> : null}
    </div>
  );
  const adb = (
    <div className={styles.adb}>
      <ol className={styles.instructions}>
        <li>{t("onboarding.capture.setup.adbInstall")}</li>
        <li>{t("onboarding.capture.setup.adbConnect")}</li>
        <li>{t("onboarding.capture.setup.adbRun")}</li>
      </ol>
      {instructions.isPending ? <StateView mode="loading" placement="inline" title={t("onboarding.capture.setup.loadingCommands")} /> : null}
      {instructions.isError || instructions.data?.adbCommands.length === 0 ? <StateView mode="error" placement="inline" title={t("onboarding.capture.setup.unavailable")}
        actions={<Button variant="secondary" onClick={() => void instructions.refetch()}>{t("common.tryAgain")}</Button>} /> : null}
      {instructions.data?.adbCommands.map((command, index) => (
        <div className={styles.command} key={index}>
          <code>{formatCaptureCommand(command)}</code>
          <Button variant="secondary" icon={copied === index ? "check" : "copy"} disabled={busy}
            aria-label={t("onboarding.capture.setup.copyNumber", { number: index + 1 })} onClick={() => copyCommand(command, index)}>
            {t(copied === index ? "onboarding.capture.setup.copied" : "onboarding.capture.setup.copy")}
          </Button>
        </div>
      ))}

    </div>
  );
  const setupAction = method === "adb" ? (
    <Button size="lg" state={busy ? "loading" : "normal"} disabled={busy} onClick={() => run(needsArming ? captureArm : captureRefresh, "verify")}>{t(needsArming ? "onboarding.capture.setup.arm" : "onboarding.capture.setup.verify.action")}</Button>
  ) : snapshot.shizuku.supported || current === 3 ? (
    <Button size="lg" state={busy ? "loading" : "normal"} disabled={busy} onClick={() => {
        if (step === "install" || step === "start") run(captureOpenShizuku);
        else run(step === "verify" && !needsArming ? captureRefresh : captureArm, "verify");
      }}>
        {t(step === "install" ? "onboarding.capture.setup.install.action" : step === "start" ? "onboarding.capture.setup.openShizuku" : step === "authorize" ? "onboarding.capture.setup.authorize.action" : needsArming ? "onboarding.capture.setup.arm" : "onboarding.capture.setup.verify.action")}
      </Button>
  ) : null;
  return (
    <div className={styles.root}>
      <Tabs equalWidth value={method} onValueChange={(value) => { if (value === "shizuku" || value === "adb") action.mutate({ nextMethod: value }); }}
        listProps={{ "aria-label": t("onboarding.capture.setup.method") }} contentClassName={styles.methodContent}
        items={[{ value: "shizuku", label: "Shizuku", disabled: busy, content: shizuku }, { value: "adb", label: "ADB", disabled: busy, content: adb }]} />
      {actionContainer ? createPortal(setupAction, actionContainer) : setupAction}
      {method === "shizuku" || instructions.data?.requiresRestart ? <p className={styles.note}>{t("onboarding.capture.setup.restart")}</p> : null}
      {action.isError ? <StateView mode="error" placement="inline" title={error ?? undefined} /> : null}
      {copied !== null ? <span role="status" className={styles.note}>{t("onboarding.capture.setup.copySuccess")}</span> : null}
      {snapshot.health.state === "granted_not_working" || action.isSuccess && action.variables?.stage === "verify" && copied === null ? (
        <StateView mode={presentation.tone === "danger" ? "error" : "info"} role={presentation.role} aria-live={presentation.urgency} placement="inline" title={snapshot.headline} description={snapshot.detail} />
      ) : null}
    </div>
  );
}

export function formatCaptureCommand(argv: readonly string[]): string {
  return argv.map((part) => /^[A-Za-z0-9_@%+=:,./-]+$/.test(part) ? part : `'${part.replace(/'/g, `'"'"'`)}'`).join(" ");
}
