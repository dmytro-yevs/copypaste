import { useLayoutEffect, useRef, useState, type ComponentProps } from "react";

import { Screen } from "@/components/layout";
import { BrandMark } from "@/components/shared/BrandMark";
import { Button, Icon } from "@/components/ui";
import { AndroidBackgroundSetup } from "@/features/capture/patterns/AndroidBackgroundSetup";
import { onboardingFlow } from "@/features/onboarding/model/onboardingFlow";
import { OnboardingPermissions } from "@/features/onboarding/patterns/OnboardingPermissions";
import { useTranslation } from "@/i18n";
import { isAndroidPlatform } from "@/lib/platform";
import { usePrefs, type OnboardingStep } from "@/store/prefs";
import { useUi, type View } from "@/store/ui";
import styles from "./OnboardingScreen.module.css";

export function OnboardingScreen(props: Omit<ComponentProps<typeof Screen>, "children">) {
  const { t } = useTranslation();
  const progress = usePrefs((state) => state.onboarding);
  const setOnboarding = usePrefs((state) => state.setOnboarding);
  const android = isAndroidPlatform();
  const [captureBusy, setCaptureBusy] = useState(false);
  const [captureReady, setCaptureReady] = useState(false);
  const [captureActions, setCaptureActions] = useState<HTMLDivElement | null>(null);
  const { step, steps, index, previous } = onboardingFlow(android, progress);
  const capturePending = step === "capture" && captureBusy;
  const viewportRef = useRef<HTMLDivElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);

  useLayoutEffect(() => {
    if (viewportRef.current) viewportRef.current.scrollTop = 0;
    headingRef.current?.focus({ preventScroll: true });
  }, [step]);

  const go = (next: OnboardingStep) => setOnboarding({ step: next });
  const finish = (view: View) => {
    usePrefs.getState().set("onboardingComplete", true);
    setOnboarding({ step: "sync" });
    useUi.getState().setView(view);
    useUi.getState().closeOnboarding();
  };

  return (
    <Screen {...props} data-onboarding-root="" data-onboarding="" data-onboarding-platform={android ? "android" : "desktop"} data-onboarding-step={step} className={styles.root}>
      <div className={styles.shell}>
        <header className={styles.topbar}>
          {previous ? <Button className={styles.back} disabled={capturePending} variant="ghost" icon="back" onClick={() => go(previous)}>{t("onboarding.back")}</Button> : <div className={styles.brand}><BrandMark size="sidebar" /><span>CopyPaste</span></div>}
          <span className={styles.stepCount}>{t("onboarding.slideLabel", { current: index + 1, total: steps.length })}</span>
        </header>
        <ol className={styles.progress} aria-label={t("onboarding.slidesLabel")}>
          {steps.map((id, position) => (
            <li key={id} aria-current={step === id ? "step" : undefined} data-done={position < index || undefined}>
              <span>{t(`onboarding.${id}.eyebrow`)}</span>
            </li>
          ))}
        </ol>
        <div className={styles.scroll} data-onboarding-scroll="" ref={viewportRef}>
          <section className={styles.page} aria-labelledby="onboarding-title" key={step}>
            {step === "welcome" ? <WelcomeArtwork /> : step === "sync" ? <SyncArtwork /> : null}
            <div className={styles.copy}>
              <span className={styles.eyebrow}>{t(`onboarding.${step}.eyebrow`)}</span>
              <h1 id="onboarding-title" ref={headingRef} tabIndex={-1}>{t(`onboarding.${step}.title`)}</h1>
              <p>{t(`onboarding.${step}.body`)}</p>
            </div>
            {step === "permissions" ? <OnboardingPermissions android={android} /> : null}
            {step === "background" ? (
              <div className={styles.explanation}>
                <div className={styles.methodSummary}><Icon name="mobile" size="lg" /><div><strong>Shizuku</strong><p>{t("onboarding.background.shizuku")}</p></div></div>
                <div className={styles.methodSummary}><Icon name="terminal" size="lg" /><div><strong>ADB</strong><p>{t("onboarding.background.adb")}</p></div></div>
                <p className={styles.note}>{t("onboarding.background.optional")}</p>
              </div>
            ) : null}
            {step === "capture" ? <AndroidBackgroundSetup onReadyChange={setCaptureReady} onBusyChange={setCaptureBusy} actionContainer={captureActions} /> : null}
          </section>
        </div>
        <footer className={styles.footer}>
          <div className={styles.actions}>
            {step === "sync" ? (
              <>
                <Button size="lg" onClick={() => finish("devices")}>{t("onboarding.sync.action")}</Button>
                <Button size="lg" variant="secondary" onClick={() => finish("history")}>{t("onboarding.sync.secondary")}</Button>
              </>
            ) : step === "background" ? (
              <>
                <Button size="lg" onClick={() => setOnboarding({ step: "capture", captureSkipped: false })}>{t("onboarding.background.action")}</Button>
                <Button size="lg" variant="secondary" disabled={capturePending} onClick={() => setOnboarding({ step: "sync", captureSkipped: true })}>{t("onboarding.skip")}</Button>
              </>
            ) : step === "capture" && !captureReady ? (
              <>
                <div className={styles.captureAction} ref={setCaptureActions} />
                <Button size="lg" variant="secondary" disabled={capturePending} onClick={() => setOnboarding({ step: "sync", captureSkipped: true })}>{t("onboarding.skip")}</Button>
              </>
            ) : (
              <Button size="lg" disabled={capturePending} onClick={() => go(step === "welcome" ? "permissions" : step === "permissions" && android ? "background" : "sync")}>
                {t(step === "welcome" ? "onboarding.welcome.action" : "onboarding.continue")}
              </Button>
            )}
          </div>
        </footer>
      </div>
    </Screen>
  );
}

function WelcomeArtwork() {
  return (
    <div className={styles.artwork} aria-hidden="true">
      <div className={styles.clipBack}><Icon name="link" size="lg" /><span /><span /></div>
      <div className={styles.clipFront}><BrandMark size="app" /><div><i /><i /><i /></div><span className={styles.clipCheck}><Icon name="check" size="md" /></span></div>
    </div>
  );
}

function SyncArtwork() {
  return (
    <div className={styles.syncArtwork} aria-hidden="true">
      <div><Icon name="laptop" size="lg" /></div><span /><Icon name="transfer" size="lg" /><span /><div><Icon name="mobile" size="lg" /></div>
    </div>
  );
}
