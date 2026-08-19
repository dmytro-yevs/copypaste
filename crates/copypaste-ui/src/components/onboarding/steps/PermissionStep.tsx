import { Bell } from "lucide-react";

import { OnboardingStepLayout } from "@/components/onboarding/OnboardingStepLayout";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { Button } from "@/components/ui/button";
import {
  useOnboardingPermissions,
  usePermissionOpenSettings,
  usePermissionRequest,
} from "@/hooks/useOnboardingPermissions";
import { useSetServiceConfig } from "@/hooks/useServiceConfig";
import { useTranslation } from "@/i18n";

export function PermissionStep({
  continue: onContinue,
  skip,
  optional,
  platform,
}: OnboardingStepProps) {
  const { t } = useTranslation();
  const snapshot = useOnboardingPermissions();
  const request = usePermissionRequest();
  const openSettings = usePermissionOpenSettings();
  const save = useSetServiceConfig();

  const status = snapshot.data?.notifications.status;
  const busy = request.isPending || openSettings.isPending || save.isPending;

  const body = bodyFor(platform, status, t("onboarding.permissions.body"), {
    windows: t("onboarding.permissions.bodyWindows"),
    denied: t("onboarding.permissions.bodyDenied"),
  });

  const primary = primaryFor(status, {
    allow: t("onboarding.permissions.action"),
    enable: t("onboarding.permissions.actionEnable"),
    retry: t("onboarding.permissions.actionRetry"),
    next: t("onboarding.permissions.actionContinue"),
  });

  return (
    <OnboardingStepLayout
      icon={Bell}
      title={t("onboarding.permissions.title")}
      body={body}
      primary={{
        label: primary.label,
        disabled: busy || (primary.kind === "allow" && snapshot.isPending),
        onClick: () => {
          if (primary.kind === "next") {
            onContinue();
            return;
          }
          if (primary.kind === "enable") {
            save.mutate({ notify_on_copy: true }, { onSuccess: () => onContinue() });
            return;
          }
          request.mutate("notifications", {
            onSuccess: (fresh) => {
              const next = fresh.notifications.status;
              if (next === "granted" || next === "not_required") {
                save.mutate({ notify_on_copy: true });
              }
            },
          });
        },
      }}
      skip={optional ? { label: t("onboarding.skip"), onClick: skip } : undefined}
    >
      {status === "denied" ? (
        <Button
          className="w-full"
          variant="outline"
          disabled={busy}
          onClick={() => openSettings.mutate("notifications")}
        >
          {t("onboarding.permissions.openSettings")}
        </Button>
      ) : null}
    </OnboardingStepLayout>
  );
}

function bodyFor(
  platform: OnboardingStepProps["platform"],
  status: string | undefined,
  fallback: string,
  variants: { windows: string; denied: string },
): string {
  if (status === "denied") return variants.denied;
  if (platform === "windows") return variants.windows;
  return fallback;
}

function primaryFor(
  status: string | undefined,
  labels: { allow: string; enable: string; retry: string; next: string },
): { kind: "allow" | "enable" | "retry" | "next"; label: string } {
  if (status === "prompt") return { kind: "allow", label: labels.allow };
  if (status === "denied") return { kind: "retry", label: labels.retry };
  if (status === "not_required") return { kind: "enable", label: labels.enable };
  return { kind: "next", label: labels.next };
}
