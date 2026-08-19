import { Link2 } from "lucide-react";

import { OnboardingStepLayout } from "@/components/onboarding/OnboardingStepLayout";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { useTranslation } from "@/i18n";

export function PairingStep({ continue: onContinue, skip, optional }: OnboardingStepProps) {
  const { t } = useTranslation();

  return (
    <OnboardingStepLayout
      icon={Link2}
      title={t("onboarding.pairing.title")}
      body={t("onboarding.pairing.body")}
      primary={{ label: t("onboarding.pairing.action"), onClick: onContinue }}
      skip={
        optional
          ? { label: t("onboarding.skip"), onClick: skip }
          : undefined
      }
    />
  );
}
