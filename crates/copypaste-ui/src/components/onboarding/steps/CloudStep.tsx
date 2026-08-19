import { Cloud } from "lucide-react";

import { OnboardingStepLayout } from "@/components/onboarding/OnboardingStepLayout";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { useTranslation } from "@/i18n";

export function CloudStep({ continue: onContinue, skip, optional }: OnboardingStepProps) {
  const { t } = useTranslation();

  return (
    <OnboardingStepLayout
      icon={Cloud}
      title={t("onboarding.cloud.title")}
      body={t("onboarding.cloud.body")}
      primary={{ label: t("onboarding.cloud.action"), onClick: onContinue }}
      skip={
        optional
          ? { label: t("onboarding.skip"), onClick: skip }
          : undefined
      }
    />
  );
}
