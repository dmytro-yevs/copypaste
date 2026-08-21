import { useMemo, useState } from "react";
import { Check, Circle, Dot } from "lucide-react";

import { DoneStep } from "@/components/onboarding/DoneStep";
import { OnboardingStepLayout } from "@/components/onboarding/OnboardingStepLayout";
import {
  onboardingPlatform,
  stepsForPlatform,
} from "@/components/onboarding/registry";
import { WelcomeStep } from "@/components/onboarding/WelcomeStep";
import { Button } from "@/components/ui/button";
import { Stepper, type StepperItem } from "@/components/ui/stepper";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";

export function OnboardingShell() {
  const { t } = useTranslation();
  const platform = onboardingPlatform();
  const steps = useMemo(() => stepsForPlatform(platform), [platform]);
  const total = steps.length + 2;
  const [index, setIndex] = useState(0);
  const stepperItems = useMemo<StepperItem[]>(
    () =>
      [
        { id: "welcome", label: t("onboarding.welcome.title") },
        ...steps.map((step) => ({
          id: step.id,
          label: t(`onboarding.${step.id}.title` as const),
        })),
        { id: "done", label: t("onboarding.done.title") },
      ].map((item, itemIndex) => ({
        id: item.id,
        label: item.label,
        icon: itemIndex < index ? Check : itemIndex === index ? Dot : Circle,
        stateLabel: t(
          itemIndex < index
            ? "capture.setup.ladder.done"
            : itemIndex === index
              ? "capture.setup.ladder.next"
              : "capture.setup.ladder.todo",
        ),
        done: itemIndex < index,
        current: itemIndex === index,
      })),
    [index, steps, t],
  );

  const finish = () => {
    usePrefs.getState().set("onboardingComplete", true);
    useUi.getState().closeOnboarding();
  };

  const goNext = () => {
    if (index >= total - 1) {
      finish();
      return;
    }
    setIndex((current) => current + 1);
  };

  const skipRemaining = () => setIndex(total - 1);

  const step = index > 0 && index < total - 1 ? steps[index - 1] : undefined;
  const Step = step?.Component;

  return (
    <div
      data-onboarding=""
      data-onboarding-step={step?.id ?? (index === 0 ? "welcome" : "done")}
      className="flex h-full min-h-0 flex-1 flex-col"
    >
      <header className="flex items-center gap-s-2 px-s-3 py-s-2">
        {index > 0 ? (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setIndex((current) => Math.max(0, current - 1))}
          >
            {t("onboarding.back")}
          </Button>
        ) : (
          <span className="inline-block min-w-[4.5rem]" aria-hidden="true" />
        )}
        <div className="flex min-w-0 flex-1 justify-center">
          <div data-onboarding-progress="" className="w-full max-w-[20rem]">
            <Stepper
              label={t("onboarding.progress", { current: index + 1, total })}
              items={stepperItems}
            />
          </div>
        </div>
        <span className="inline-block min-w-[4.5rem]" aria-hidden="true" />
      </header>

      <div className="flex min-h-0 flex-1 items-center justify-center overflow-y-auto">
        <div
          className={cn(
            "flex w-full justify-center rounded-xl border border-border bg-card shadow-sm",
            "mx-s-3 max-w-[32rem]",
          )}
        >
          {index === 0 ? (
            <WelcomeStep onContinue={goNext} onSkipSetup={finish} />
          ) : index >= total - 1 ? (
            <DoneStep onFinish={finish} />
          ) : Step && step ? (
            <Step
              id={step.id}
              platform={platform}
              optional={step.optional}
              index={index}
              total={total}
              continue={goNext}
              skip={goNext}
              skipRemaining={skipRemaining}
            />
          ) : (
            <OnboardingStepLayout
              title={t("onboarding.done.title")}
              primary={{ label: t("onboarding.continue"), onClick: goNext }}
            />
          )}
        </div>
      </div>
    </div>
  );
}
