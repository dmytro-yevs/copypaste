import { Button, Icon, Surface } from "@/components/ui";
import type { ConnectionSummaryPresentation } from "@/features/devices/model/devicePresentation";
import styles from "./ConnectionSummary.module.css";

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
  return (
    <Surface asChild elevation="raised" border="subtle" radius="md">
      <section
        data-slot="status-card"
        data-state={summary.state}
        className={styles.root}
        role="status"
        aria-busy={summary.busy || undefined}
      >
        <div className={styles.layout}>
          <span className={styles.indicator} aria-hidden="true">
            <Icon
              name={
                summary.state === "attention"
                  ? "alert"
                  : summary.state === "connected"
                    ? "checkCircle"
                    : "refresh"
              }
              size="sm"
            />
          </span>
          <span className={styles.copy}>
            <strong>{summary.title}</strong>
            {summary.supportingLine ? <small>{summary.supportingLine}</small> : null}
          </span>
          {summary.action ? (
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
          ) : null}
        </div>
      </section>
    </Surface>
  );
}
