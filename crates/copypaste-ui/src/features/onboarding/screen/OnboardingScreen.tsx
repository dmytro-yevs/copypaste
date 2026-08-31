import { useState, type ComponentProps, type ReactNode } from "react";

import { Screen } from "@/components/layout";
import { BrandMark } from "@/components/shared/BrandMark";
import { Button } from "@/components/ui";
import { MAX_PAIRINGS } from "@/features/devices/model";
import {
  CaptureArtwork,
  NetworkArtwork,
  WelcomeArtwork,
} from "@/features/onboarding/components/OnboardingArtwork";
import { AndroidCaptureSetup } from "@/features/onboarding/patterns/AndroidCaptureSetup";
import { useTranslation } from "@/i18n";
import { isAndroidPlatform } from "@/lib/platform";
import { usePrefs } from "@/store/prefs";
import { useUi, type View } from "@/store/ui";
import styles from "./OnboardingScreen.module.css";

export const ONBOARDING_SLIDE_IDS = ["welcome", "capture", "connections"] as const;
const SLIDE_COUNT = ONBOARDING_SLIDE_IDS.length;

export function OnboardingScreen(props: Omit<ComponentProps<typeof Screen>, "children">) {
  const { t } = useTranslation();
  const [index, setIndex] = useState(0);
  const android = isAndroidPlatform();

  const finish = (view: View = "history") => {
    usePrefs.getState().set("onboardingComplete", true);
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
            onClick={() => setIndex(dotIndex)}
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
        <div className={styles.window}>
          {index === 0 ? (
            <OnboardingSlide
              eyebrow={t("onboarding.welcome.eyebrow")}
              title={t("onboarding.welcome.title")}
              body={t("onboarding.welcome.body")}
              artwork={<WelcomeArtwork />}
              pagination={pagination}
              lockup
              primary={{ label: t("onboarding.welcome.action"), onClick: () => setIndex(1) }}
              secondary={{ label: t("onboarding.welcome.secondary"), onClick: () => finish() }}
            />
          ) : index === 1 ? (
            <OnboardingSlide
              eyebrow={t("onboarding.capture.eyebrow")}
              title={t("onboarding.capture.title")}
              body={t(android ? "onboarding.capture.androidBody" : "onboarding.capture.body")}
              artwork={android ? <AndroidCaptureSetup /> : <CaptureArtwork />}
              artworkInteractive={android}
              pagination={pagination}
              primary={{
                label: t(android ? "onboarding.continue" : "onboarding.capture.action"),
                onClick: () => setIndex(2),
              }}
              secondary={{ label: t("onboarding.skip"), onClick: () => setIndex(2) }}
            />
          ) : (
            <OnboardingSlide
              eyebrow={t("onboarding.connections.eyebrow")}
              title={t("onboarding.connections.title")}
              body={t("onboarding.connections.body", { pairingLimit: MAX_PAIRINGS })}
              artwork={<NetworkArtwork pairingLimit={MAX_PAIRINGS} />}
              pagination={pagination}
              primary={{ label: t("onboarding.connections.action"), onClick: () => finish("devices") }}
              secondary={{ label: t("onboarding.connections.secondary"), onClick: () => finish() }}
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
  artwork,
  pagination,
  artworkInteractive = false,
  lockup = false,
  primary,
  secondary,
}: {
  eyebrow: string;
  title: string;
  body: string;
  artwork: ReactNode;
  pagination: ReactNode;
  artworkInteractive?: boolean;
  lockup?: boolean;
  primary: { label: string; onClick: () => void };
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
        <h1>{title}</h1>
        <p>{body}</p>
        <div className={styles.actions}>
          {pagination}
          <Button size="md" onClick={primary.onClick}>
            {primary.label}
          </Button>
          <Button size="md" variant="secondary" onClick={secondary.onClick}>
            {secondary.label}
          </Button>
        </div>
      </div>
      <div className={styles.art} data-interactive={artworkInteractive || undefined}>
        {artwork}
      </div>
    </section>
  );
}
