import { useId } from "react";
import { Icon } from "@/components/ui/icon";
import { FieldFeedback, SettingsRow } from "@/components/shared";
import { Badge, Button, Switch } from "@/components/ui";
import { primaryOf } from "@/features/capture/model";
import { Section } from "@/features/settings/components/Section";
import { useCaptureMutation, useCaptureState } from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { captureArm, captureRefresh, captureSetEnabled } from "@/lib/ipc";
import styles from "./ServiceCaptureControls.module.css";

export function ServiceCaptureControls() {
  const { t } = useTranslation();
  const capture = useCaptureState();
  const run = useCaptureMutation();
  const snapshot = capture.data;
  const descriptionId = useId();
  const toggleDescriptionId = useId();

  if (snapshot === undefined) {
    return (
      <Section title={t("capture.title")}>
        <SettingsRow title={t("capture.loading.title")}>
          {capture.isError ? (
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void capture.refetch()}
            >
              <Icon name="refresh" aria-hidden="true" />
              {t("common.tryAgain")}
            </Button>
          ) : (
            <FieldFeedback state="pending">
              Checking capture…
            </FieldFeedback>
          )}
        </SettingsRow>
      </Section>
    );
  }

  const primary = primaryOf(snapshot.nextStep);
  const canToggle = snapshot.rung !== "desktop" && snapshot.shizuku.supported;
  const enabled = snapshot.health.state !== "disabled";
  const action =
    primary === "none"
      ? undefined
      : () =>
          run.mutate(
            primary === "recheck" ? () => captureRefresh() : () => captureArm(),
          );
  const actionLabel =
    primary === "arm"
      ? t("capture.setup.action.arm")
      : primary === "permission"
        ? t("capture.setup.action.permission")
        : t("capture.setup.action.checkAgain");

  return (
    <Section title={t("capture.title")}>
      <SettingsRow
        title={snapshot.headline}
        description={snapshot.detail ?? undefined}
        descriptionId={descriptionId}
        note={
          run.isError ? (
            <FieldFeedback state="error">
              Capture couldn’t be changed.
            </FieldFeedback>
          ) : undefined
        }
      >
        {action ? (
          <Button
            disabled={run.isPending}
            aria-busy={run.isPending || undefined}
            aria-describedby={snapshot.detail ? descriptionId : undefined}
            onClick={action}
          >
            <Icon name="refresh"
              aria-hidden="true"
              className={run.isPending ? styles.spinner : undefined}
            />
            {run.isPending ? t("capture.setup.action.busy") : actionLabel}
          </Button>
        ) : (
          <Badge
            variant={snapshot.health.state === "working" ? "ok" : "secondary"}
          >
            {snapshot.health.state === "working" ? "On" : "Off"}
          </Badge>
        )}
      </SettingsRow>
      {canToggle ? (
        <SettingsRow
          title={t("capture.setup.enable.title")}
          description={t("capture.setup.enable.body")}
          descriptionId={toggleDescriptionId}
        >
          <Switch
            aria-label={t("capture.setup.enable.title")}
            aria-describedby={toggleDescriptionId}
            aria-busy={run.isPending || undefined}
            checked={enabled}
            disabled={run.isPending}
            onCheckedChange={(next) =>
              run.mutate(() => captureSetEnabled(next))
            }
          />
        </SettingsRow>
      ) : null}
    </Section>
  );
}
