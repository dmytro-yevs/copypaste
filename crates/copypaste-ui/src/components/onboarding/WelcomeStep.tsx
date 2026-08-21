import { ClipboardList } from "lucide-react";

import { Button } from "@/components/ui/button";
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
    <section className="flex w-full max-w-[28rem] flex-col px-s-5 py-s-6 sm:px-s-7 sm:py-s-7">
      <div className="flex flex-1 flex-col items-center justify-center text-center">
        <div className="relative mb-s-5" aria-hidden="true">
          <span className="absolute -right-s-2 -top-s-2 size-8 rounded-lg border border-border bg-panel" />
          <span className="absolute -bottom-s-2 -left-s-2 size-10 rounded-xl border border-border bg-secondary" />
          <span className="relative flex size-16 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-sm">
            <ClipboardList size={30} strokeWidth={1.8} />
          </span>
        </div>

        <h1 className="text-balance text-2xl font-semibold tracking-tight text-foreground">
          {t("onboarding.welcome.title")}
        </h1>
        <p className="mt-s-3 max-w-[24rem] text-pretty text-sm leading-relaxed text-muted-foreground">
          {t("onboarding.welcome.body")}
        </p>
      </div>

      <div className="mt-s-6 flex w-full flex-col items-center gap-s-2">
        <Button className="w-full" size="lg" onClick={onContinue}>
          {t("onboarding.welcome.action")}
        </Button>
        <Button className="w-full" variant="ghost" onClick={onSkipSetup}>
          {t("onboarding.skipSetup")}
        </Button>
      </div>
    </section>
  );
}
