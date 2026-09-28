import { StateView } from "@/components/shared/StateView";
import { Button } from "@/components/ui";
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
      icon={summary.action.icon}
      disabled={actionDisabled}
      onClick={onAction}
    >
      {actionBusy ? t("devices.actions.syncing") : actionLabel ?? summary.action.label}
    </Button>
  ) : undefined;

  const mode = summary.status === "positive"
    ? "success"
    : summary.status === "attention"
      ? "warning"
      : "info";

  return (
    <StateView
      mode={mode}
      placement="inline"
      title={summary.title}
      description={summary.supportingLine}
      icon={summary.icon}
      actions={action}
      role="status"
      aria-live={summary.live}
      aria-busy={summary.busy || undefined}
    />
  );
}
