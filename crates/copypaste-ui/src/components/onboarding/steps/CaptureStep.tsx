import { Smartphone } from "lucide-react";

import { OnboardingStepLayout } from "@/components/onboarding/OnboardingStepLayout";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { useTranslation } from "@/i18n";

export function CaptureStep({ continue: onContinue, skip, optional }: OnboardingStepProps) {
  const { t } = useTranslation();

  return (
    <OnboardingStepLayout
      icon={Smartphone}
      title={t("onboarding.capture.title")}
      body={t("onboarding.capture.body")}
      primary={{ label: t("onboarding.capture.action"), onClick: onContinue }}
      skip={
        optional
          ? { label: t("onboarding.skip"), onClick: skip }
          : undefined
      }
    />
  );
}
