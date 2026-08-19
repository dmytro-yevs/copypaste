import { Smartphone } from "lucide-react";

import { OnboardingStepLayout } from "@/components/onboarding/OnboardingStepLayout";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { Button } from "@/components/ui/button";
import {
  useOnboardingPermissions,
  usePermissionRequest,
} from "@/hooks/useOnboardingPermissions";
import { useCaptureMutation, useCaptureNow, useCaptureState } from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { captureArm } from "@/lib/ipc";

export function CaptureStep({ continue: onContinue, skip, optional }: OnboardingStepProps) {
  const { t } = useTranslation();
  const capture = useCaptureState();
  const now = useCaptureNow();
  const arm = useCaptureMutation();
  const permissions = useOnboardingPermissions();
  const requestTile = usePermissionRequest();

  const tile = permissions.data?.tile;
  const showTile = tile !== undefined && tile.status !== "unavailable";
  const showBackground = capture.data !== undefined && capture.data.rung !== "desktop";
  const busy = now.isPending || arm.isPending || requestTile.isPending;

  return (
    <OnboardingStepLayout
      icon={Smartphone}
      title={t("onboarding.capture.title")}
      body={t("onboarding.capture.body")}
      primary={{
        label: t("onboarding.continue"),
        disabled: busy,
        onClick: onContinue,
      }}
      skip={optional ? { label: t("onboarding.skip"), onClick: skip } : undefined}
    >
      <div className="flex flex-col gap-s-2">
        <Button
          className="w-full"
          variant="outline"
          disabled={busy}
          onClick={() => now.mutate("in_app")}
        >
          {t("onboarding.capture.saveNow")}
        </Button>
        {showTile ? (
          <Button
            className="w-full"
            variant="outline"
            disabled={busy || tile.status === "granted"}
            onClick={() => requestTile.mutate("tile")}
          >
            {tile.status === "granted"
              ? t("onboarding.capture.tileAdded")
              : t("onboarding.capture.addTile")}
          </Button>
        ) : null}
        {showBackground ? (
          <Button
            className="w-full"
            variant="outline"
            disabled={busy}
            onClick={() => arm.mutate(() => captureArm())}
          >
            {t("onboarding.capture.background")}
          </Button>
        ) : null}
      </div>
    </OnboardingStepLayout>
  );
}
