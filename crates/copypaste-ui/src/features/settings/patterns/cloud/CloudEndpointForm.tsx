import { useId, type FormEvent } from "react";

import { StateView } from "@/components/shared/StateView";
import { Button, Input, Label } from "@/components/ui";
import type { CloudAccountController } from "@/features/settings/hooks/useCloudAccountController";
import { useTranslation } from "@/i18n";
import styles from "../CloudSyncSettings.module.css";

export function CloudEndpointForm({
  controller,
  replacing,
}: {
  controller: CloudAccountController;
  replacing: boolean;
}) {
  const { t } = useTranslation();
  const urlId = useId();
  const keyId = useId();
  const helpId = useId();
  const warningId = useId();
  const feedbackId = useId();

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    controller.configureEndpoint();
  }

  const describedBy = replacing
    ? `${helpId} ${warningId} ${feedbackId}`
    : `${helpId} ${feedbackId}`;

  return (
    <form
      className={styles.form}
      aria-label={t("settings.sync.cloud.endpoint.formLabel")}
      aria-describedby={describedBy}
      onSubmit={submit}
    >
      {replacing ? (
        <div id={warningId}>
          <StateView mode="warning" placement="inline" title={t("settings.sync.cloud.endpoint.changeWarning")} />
        </div>
      ) : null}
      <div
        className={styles.field}
        data-settings-search-target={`row:${t("settings.sync.cloud.endpoint.url")}`}
      >
        <Label htmlFor={urlId}>{t("settings.sync.cloud.endpoint.url")}</Label>
        <Input
          id={urlId}
          type="url"
          inputMode="url"
          value={controller.endpointDraft.url}
          onChange={(event) => controller.updateEndpoint("url", event.target.value)}
          autoComplete="url"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          disabled={controller.busy}
          required
        />
      </div>
      <div
        className={styles.field}
        data-settings-search-target={`row:${t("settings.sync.cloud.endpoint.publishableKey")}`}
      >
        <Label htmlFor={keyId}>{t("settings.sync.cloud.endpoint.publishableKey")}</Label>
        <Input
          id={keyId}
          type="text"
          value={controller.endpointDraft.anonKey}
          onChange={(event) => controller.updateEndpoint("anonKey", event.target.value)}
          autoComplete="off"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          disabled={controller.busy}
          required
        />
      </div>
      <p id={helpId} className={styles.help}>
        {t("settings.sync.cloud.endpoint.help")}
        {replacing ? ` ${t("settings.sync.cloud.endpoint.restoreHelp")}` : ""}
      </p>
      <div id={feedbackId} className={styles.feedback}>
        {controller.endpointPending ? (
          <StateView mode="loading" placement="control" title={t("settings.sync.cloud.endpoint.saving")} />
        ) : controller.endpointError ? (
          <StateView mode="error" placement="control" title={t("settings.sync.cloud.endpoint.error")} />
        ) : controller.endpointDirty ? (
          <span className={styles.dirty}>{t("settings.sync.cloud.endpoint.unsaved")}</span>
        ) : null}
      </div>
      <div className={styles.formActions}>
        {replacing ? (
          <>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={controller.busy}
              onClick={controller.restoreHostedEndpoint}
            >
              {t("settings.sync.cloud.endpoint.restore")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              disabled={controller.busy}
              onClick={controller.cancelEndpointEdit}
            >
              {t("settings.sync.cloud.cancel")}
            </Button>
          </>
        ) : (
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={controller.busy || !controller.endpointDirty}
            onClick={controller.clearEndpoint}
          >
            {t("settings.sync.cloud.discard")}
          </Button>
        )}
        <Button
          type="submit"
          size="sm"
          pending={controller.endpointPending}
          disabled={controller.busy || !controller.endpointComplete}
        >
          {t(controller.endpointPending
            ? "settings.sync.cloud.endpoint.saving"
            : replacing
              ? "settings.sync.cloud.endpoint.save"
              : "settings.sync.cloud.endpoint.configure")}
        </Button>
      </div>
    </form>
  );
}
