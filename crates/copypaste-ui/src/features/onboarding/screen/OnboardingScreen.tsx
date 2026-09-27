import { useLayoutEffect, useRef, useState, type ComponentProps, type ReactNode, type Ref } from "react";

import { Screen } from "@/components/layout";
import { BrandMark } from "@/components/shared/BrandMark";
import { Button } from "@/components/ui";
import { CaptureSetupState } from "@/features/capture";
import { PairingLauncherDialog } from "@/features/devices/patterns/PairingLauncherDialog";
import { AndroidCaptureSetup } from "@/features/onboarding/patterns/AndroidCaptureSetup";
import { usePairing } from "@/features/pairing";
import { ClipboardNotificationSection } from "@/features/settings/patterns/service/ClipboardServiceSections";
import { PrivacyServiceSections } from "@/features/settings/patterns/service/PrivacyServiceSections";
import { ServiceSettingsProvider } from "@/features/settings/patterns/service/ServiceSettingsController";
import { CloudSyncSettings } from "@/features/settings/patterns/CloudSyncSettings";
import { PrivacyDisplaySettings } from "@/features/settings/patterns/ListTab";
import { SwitchRow } from "@/features/settings/components/SwitchRow";
import { settingsCapabilities } from "@/features/settings/model/settingsNavigation";
import { useOpenAtLogin, useSetOpenAtLogin } from "@/hooks/useOpenAtLogin";
import { useSetServiceConfig } from "@/hooks/useServiceConfig";
import { useTranslation } from "@/i18n";
import { currentPlatform, isAndroidPlatform } from "@/lib/platform";
import {
  ONBOARDING_STEPS,
  type OnboardingStep,
  type OnboardingSyncChoice,
  usePrefs,
} from "@/store/prefs";
import { useUi, type View } from "@/store/ui";
import styles from "./OnboardingScreen.module.css";

export const ONBOARDING_SLIDE_IDS = ONBOARDING_STEPS;
const SLIDE_COUNT = ONBOARDING_SLIDE_IDS.length;

export function OnboardingScreen(props: Omit<ComponentProps<typeof Screen>, "children">) {
  const { t } = useTranslation();
  const progress = usePrefs((state) => state.onboarding);
  const setOnboarding = usePrefs((state) => state.setOnboarding);
  const [syncSaving, setSyncSaving] = useState(false);
  const index = ONBOARDING_SLIDE_IDS.indexOf(progress.step);
  const android = isAndroidPlatform();
  const viewportRef = useRef<HTMLDivElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);

  useLayoutEffect(() => {
    if (viewportRef.current) viewportRef.current.scrollTop = 0;
    headingRef.current?.focus({ preventScroll: true });
  }, [progress.step]);

  const go = (step: OnboardingStep) => setOnboarding({ step });

  const finish = (view: View = "history") => {
    usePrefs.getState().set("onboardingComplete", true);
    setOnboarding({ step: "complete" });
    useUi.getState().setView(view);
    useUi.getState().closeOnboarding();
  };

  const pagination = (
    <div className={styles.dotsPosition}>
      <nav className={styles.dots} aria-label={t("onboarding.slidesLabel")}>
        {Array.from({ length: SLIDE_COUNT }, (_, dotIndex) => (
          <Button
            key={dotIndex}
            type="button"
            variant="ghost"
            size="compactIcon"
            className={styles.dot}
            aria-label={t("onboarding.slideLabel", { current: dotIndex + 1, total: SLIDE_COUNT })}
            aria-current={dotIndex === index ? "step" : undefined}
            onClick={() => go(ONBOARDING_SLIDE_IDS[dotIndex])}
          />
        ))}
      </nav>
    </div>
  );

  return (
    <Screen
      {...props}
      data-onboarding-root=""
      data-onboarding=""
      data-onboarding-step={ONBOARDING_SLIDE_IDS[index]}
      className={styles.root}
    >
      <div className={styles.stage}>
        <div className={styles.window} data-onboarding-scroll="" ref={viewportRef}>
          {progress.step === "welcome" ? (
            <OnboardingSlide
              eyebrow={t("onboarding.welcome.eyebrow")}
              title={t("onboarding.welcome.title")}
              body={t("onboarding.welcome.body")}
              headingRef={headingRef}
              content={<WelcomePanel />}
              pagination={pagination}
              lockup
              primary={{ label: t("onboarding.welcome.action"), onClick: () => go("capture") }}
              secondary={{ label: t("onboarding.welcome.secondary"), onClick: () => finish() }}
            />
          ) : progress.step === "capture" ? (
            <OnboardingSlide
              eyebrow={t("onboarding.capture.eyebrow")}
              title={t("onboarding.capture.title")}
              body={t(android ? "onboarding.capture.androidBody" : "onboarding.capture.body")}
              headingRef={headingRef}
              content={android ? <AndroidCaptureOnboarding /> : <CaptureSetupState />}
              contentInteractive
              pagination={pagination}
              primary={{
                label: t("onboarding.continue"),
                onClick: () => go("privacy"),
              }}
              secondary={{
                label: t("onboarding.skip"),
                onClick: () => {
                  setOnboarding({ captureSkipped: true });
                  go("privacy");
                },
              }}
            />
          ) : progress.step === "privacy" ? (
            <OnboardingSlide
              eyebrow={t("onboarding.privacy.eyebrow")}
              title={t("onboarding.privacy.title")}
              body={t("onboarding.privacy.body")}
              headingRef={headingRef}
              content={<PrivacyAndBasics />}
              contentInteractive
              pagination={pagination}
              primary={{ label: t("onboarding.privacy.action"), onClick: () => go("sync") }}
              secondary={{
                label: t("onboarding.privacy.secondary"),
                onClick: () => {
                  setOnboarding({ privacySkipped: true });
                  go("sync");
                },
              }}
            />
          ) : progress.step === "sync" ? (
            <OnboardingSlide
              eyebrow={t("onboarding.sync.eyebrow")}
              title={t("onboarding.sync.title")}
              body={t("onboarding.sync.body")}
              headingRef={headingRef}
              content={<SyncSetup
                choice={progress.syncChoice}
                onChoose={(syncChoice) => setOnboarding({ syncChoice })}
                onPendingChange={setSyncSaving}
              />}
              contentInteractive
              pagination={pagination}
              primary={{ label: t("onboarding.sync.action"), onClick: () => go("complete"), disabled: syncSaving }}
              secondary={{ label: t("onboarding.back"), onClick: () => go("privacy") }}
            />
          ) : (
            <OnboardingSlide
              eyebrow={t("onboarding.complete.eyebrow")}
              title={t("onboarding.complete.title")}
              body={t("onboarding.complete.body")}
              headingRef={headingRef}
              content={<CompletionPanel />}
              pagination={pagination}
              primary={{ label: t("onboarding.complete.action"), onClick: () => finish() }}
              secondary={{ label: t("onboarding.complete.secondary"), onClick: () => go("sync") }}
            />
          )}

        </div>
      </div>
    </Screen>
  );
}

function OnboardingSlide({
  eyebrow,
  title,
  body,
  headingRef,
  content,
  pagination,
  contentInteractive = false,
  lockup = false,
  primary,
  secondary,
}: {
  eyebrow: string;
  title: string;
  body: string;
  headingRef: Ref<HTMLHeadingElement>;
  content: ReactNode;
  pagination: ReactNode;
  contentInteractive?: boolean;
  lockup?: boolean;
  primary: { label: string; onClick: () => void; disabled?: boolean };
  secondary: { label: string; onClick: () => void };
}) {
  return (
    <section className={styles.slide}>
      <div className={styles.copy}>
        {lockup ? (
          <div className={styles.lockup}>
            <span className={styles.lockupLayout}>
              <span className={styles.lockupMark}><BrandMark size="app" animated /></span>
              <span className={styles.lockupName}>
                <strong>CopyPaste</strong>
                <small>Private clipboard memory</small>
              </span>
            </span>
          </div>
        ) : null}
        <span className={styles.eyebrow}>{eyebrow}</span>
        <h1 ref={headingRef} tabIndex={-1}>{title}</h1>
        <p>{body}</p>
        <div className={styles.actions}>
          {pagination}
          <Button size="md" disabled={primary.disabled} onClick={primary.onClick}>
            {primary.label}
          </Button>
          <Button size="md" variant="secondary" onClick={secondary.onClick}>
            {secondary.label}
          </Button>
        </div>
      </div>
      <div className={styles.art} data-interactive={contentInteractive || undefined}>
        {content}
      </div>
    </section>
  );
}

function WelcomePanel() {
  return <BrandMark size="app" animated />;
}

function AndroidCaptureOnboarding() {
  return (
    <>
      <CaptureSetupState />
      <AndroidCaptureSetup />
    </>
  );
}

function PrivacyAndBasics() {
  const capabilities = settingsCapabilities(currentPlatform());

  return (
    <div>
      <PrivacyDisplaySettings ready supportsScreenshots={capabilities.screenshots} />
      <ServiceSettingsProvider requiresPrivateMode>
        <PrivacyServiceSections />
        {!isAndroidPlatform() ? (
          <ClipboardNotificationSection supportsCopyNotifications={capabilities.copyNotifications} />
        ) : null}
      </ServiceSettingsProvider>
      {capabilities.startup ? <StartupOption /> : null}
    </div>
  );
}

function StartupOption() {
  const { t } = useTranslation();
  const startup = useOpenAtLogin();
  const saveStartup = useSetOpenAtLogin();
  const unavailable = startup.isError || saveStartup.isError;

  return (
    <SwitchRow
      id="onboarding-open-at-login"
      title={t("onboarding.startup.title")}
      checked={startup.data ?? false}
      disabled={startup.isPending || unavailable || saveStartup.isPending}
      busy={startup.isPending || saveStartup.isPending}
      note={startup.isPending
        ? t("onboarding.startup.checking")
        : unavailable ? t("onboarding.startup.unavailable") : undefined}
      onChange={(open) => saveStartup.mutate(open)}
    />
  );
}

function SyncSetup({
  choice,
  onChoose,
  onPendingChange,
}: {
  choice: OnboardingSyncChoice;
  onChoose: (choice: OnboardingSyncChoice) => void;
  onPendingChange: (pending: boolean) => void;
}) {
  const { t } = useTranslation();
  const [pairingOpen, setPairingOpen] = useState(false);
  const pairing = usePairing();
  const syncConfig = useSetServiceConfig();
  const [saveFailed, setSaveFailed] = useState(false);
  const lanSelected = choice === "lan" || choice === "both";
  const cloudSelected = choice === "cloud" || choice === "both";

  const choose = (next: Exclude<OnboardingSyncChoice, null>) => {
    const config = next === "lan" || next === "both"
      ? { sync_enabled: true, lan_visibility: true }
      : next === "cloud"
        ? { sync_enabled: true, lan_visibility: false }
        : { sync_enabled: false, lan_visibility: false };
    setSaveFailed(false);
    onPendingChange(true);
    syncConfig.mutate(config, {
      onSuccess: () => {
        onChoose(next);
        onPendingChange(false);
      },
      onError: () => {
        setSaveFailed(true);
        onPendingChange(false);
      },
    });
  };

  return (
    <div>
      <div className={styles.syncChoices} role="radiogroup" aria-label={t("onboarding.sync.eyebrow")}>
        {([
          ["lan", "onboarding.sync.lan", "onboarding.sync.lanDetail"],
          ["cloud", "onboarding.sync.cloud", "onboarding.sync.cloudDetail"],
          ["both", "onboarding.sync.both", "onboarding.sync.bothDetail"],
          ["later", "onboarding.sync.later", "onboarding.sync.laterDetail"],
        ] as const).map(([value, label, detail]) => (
          <Button
            key={value}
            type="button"
            variant="secondary"
            className={styles.syncChoice}
            role="radio"
            aria-checked={choice === value}
            disabled={syncConfig.isPending}
            onClick={() => choose(value)}
          >
            <span><strong>{t(label)}</strong><small>{t(detail)}</small></span>
          </Button>
        ))}
      </div>
      {lanSelected ? (
        <Button type="button" variant="secondary" onClick={() => setPairingOpen(true)}>
          {t("onboarding.sync.setupLan")}
        </Button>
      ) : null}
      {cloudSelected ? <CloudSyncSettings /> : null}
      {choice !== null ? <p className={styles.savedChoice}>{t("onboarding.sync.selectionSaved")}</p> : null}
      {saveFailed ? <p className={styles.savedChoice} role="alert">{t("onboarding.sync.saveFailed")}</p> : null}
      <PairingLauncherDialog
        open={pairingOpen}
        available={pairing.protectedPresentationAvailable || pairing.webPreview}
        preview={pairing.webPreview}
        disabled={pairing.isChecking || pairing.isPending}
        pairing={pairing}
        onOpenChange={setPairingOpen}
        onCreate={() => pairing.run("create")}
        onJoin={() => pairing.run("join")}
      />
    </div>
  );
}

function CompletionPanel() {
  return <BrandMark size="app" animated />;
}
