import { ClipboardList } from "lucide-react";

import { OnboardingStepLayout } from "@/components/onboarding/OnboardingStepLayout";
import { useTranslation } from "@/i18n";

export function WelcomeStep({
  onContinue,
  onSkipSetup,
}: {
  onContinue: () => void;
  onSkipSetup: () => void;
}) {
  const { t } = useTranslation();

  return (
    <OnboardingStepLayout
      icon={ClipboardList}
      title={t("onboarding.welcome.title")}
      body={t("onboarding.welcome.body")}
      primary={{ label: t("onboarding.welcome.action"), onClick: onContinue }}
      skip={{ label: t("onboarding.skipSetup"), onClick: onSkipSetup }}
    />
  );
}
