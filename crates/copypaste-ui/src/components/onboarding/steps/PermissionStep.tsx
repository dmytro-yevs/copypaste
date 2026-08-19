import { Shield } from "lucide-react";

import { OnboardingStepLayout } from "@/components/onboarding/OnboardingStepLayout";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { useTranslation } from "@/i18n";

export function PermissionStep({ continue: onContinue, skip, optional }: OnboardingStepProps) {
  const { t } = useTranslation();

  return (
    <OnboardingStepLayout
      icon={Shield}
      title={t("onboarding.permissions.title")}
      body={t("onboarding.permissions.body")}
      primary={{ label: t("onboarding.permissions.action"), onClick: onContinue }}
      skip={
        optional
          ? { label: t("onboarding.skip"), onClick: skip }
          : undefined
      }
    />
  );
}
