import { StatusCard } from "@/components/shared";
import { Button, Icon } from "@/components/ui";
import type { ConnectionSummaryPresentation } from "@/features/devices/model/devicePresentation";
import { t } from "@/i18n";

export function ConnectionSummary({
  summary,
  actionLabel,
  actionDisabled,
  actionBusy,
  onAction,
}: {
  summary: ConnectionSummaryPresentation;
  actionLabel?: string;
  actionDisabled: boolean;
  actionBusy: boolean;
  onAction: () => void;
}) {
  const action = summary.action ? (
    <Button
      type="button"
      variant="secondary"
      size="compact"
      disabled={actionDisabled}
      onClick={onAction}
    >
      <Icon
        name={summary.action.icon}
        aria-hidden="true"
      />
      {actionBusy ? t("devices.actions.syncing") : actionLabel ?? summary.action.label}
    </Button>
  ) : undefined;

  return (
    <StatusCard
      status={summary.status}
      title={summary.title}
      detail={summary.supportingLine}
      icon={summary.icon}
      action={action}
      density="compact"
      busy={summary.busy}
      live={summary.live}
    />
  );
}
