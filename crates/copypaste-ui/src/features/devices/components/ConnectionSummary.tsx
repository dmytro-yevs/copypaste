import { StatusCard, type StatusCardStatus } from "@/components/shared";
import { Button, Icon, type IconName } from "@/components/ui";
import type { ConnectionSummaryPresentation } from "@/features/devices/model/devicePresentation";

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
  const status: StatusCardStatus = summary.state === "connected"
    ? "positive"
    : summary.state === "syncing"
      ? "info"
      : "attention";
  const icon: IconName = summary.state === "connected"
    ? "checkCircle"
    : summary.state === "syncing"
      ? "refresh"
      : "alert";
  const action = summary.action ? (
    <Button
      type="button"
      variant="secondary"
      size="compact"
      disabled={actionDisabled}
      onClick={onAction}
    >
      <Icon
        name={summary.action.kind === "retry-peer" ? "refresh" : "devices"}
        aria-hidden="true"
      />
      {actionBusy ? "Syncing…" : actionLabel ?? summary.action.label}
    </Button>
  ) : undefined;

  return (
    <StatusCard
      status={status}
      title={summary.title}
      detail={summary.supportingLine}
      icon={icon}
      action={action}
      density="compact"
      busy={summary.busy}
    />
  );
}
