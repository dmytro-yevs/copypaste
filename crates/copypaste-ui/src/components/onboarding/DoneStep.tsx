import { Check } from "lucide-react";

import { OnboardingStepLayout } from "@/components/onboarding/OnboardingStepLayout";
import { useTranslation } from "@/i18n";

export function DoneStep({ onFinish }: { onFinish: () => void }) {
  const { t } = useTranslation();

  return (
    <OnboardingStepLayout
      icon={Check}
      title={t("onboarding.done.title")}
      body={t("onboarding.done.body")}
      primary={{ label: t("onboarding.done.action"), onClick: onFinish }}
    />
  );
}
