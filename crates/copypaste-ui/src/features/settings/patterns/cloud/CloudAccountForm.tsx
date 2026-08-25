import { useId, type FormEvent } from "react";

import { Icon } from "@/components/ui/icon";
import { FieldFeedback } from "@/components/shared";
import { Button, Input, Label } from "@/components/ui";
import type { CloudAccountController } from "@/features/settings/hooks/useCloudAccountController";
import { useTranslation } from "@/i18n";
import styles from "../CloudSyncSettings.module.css";

export function CloudAccountForm({ controller }: { controller: CloudAccountController }) {
  const { t } = useTranslation();
  const emailId = useId();
  const passwordId = useId();
  const passphraseId = useId();
  const passphraseHelpId = useId();
  const feedbackId = useId();

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    controller.submitSignIn();
  }

  return (
    <form
      className={styles.form}
      aria-label={t("settings.sync.cloud.formLabel")}
      aria-describedby={feedbackId}
      onSubmit={submit}
    >
      <div className={styles.field}>
        <Label htmlFor={emailId}>{t("settings.sync.cloud.email")}</Label>
        <Input
          id={emailId}
          type="email"
          value={controller.accountDraft.email}
          onChange={(event) => controller.updateAccount("email", event.target.value)}
          autoComplete="username"
          disabled={controller.busy}
          required
        />
      </div>
      <div className={styles.field}>
        <Label htmlFor={passwordId}>{t("settings.sync.cloud.password")}</Label>
        <Input
          id={passwordId}
          type="password"
          value={controller.accountDraft.password}
          onChange={(event) => controller.updateAccount("password", event.target.value)}
          autoComplete="current-password"
          disabled={controller.busy}
          required
        />
      </div>
      <div className={styles.field}>
        <Label htmlFor={passphraseId}>{t("settings.sync.cloud.passphrase")}</Label>
        <Input
          id={passphraseId}
          type="password"
          value={controller.accountDraft.passphrase}
          onChange={(event) => controller.updateAccount("passphrase", event.target.value)}
          autoComplete="off"
          aria-describedby={passphraseHelpId}
          disabled={controller.busy}
          required
        />
        <span id={passphraseHelpId} className={styles.help}>
          {t("settings.sync.cloud.passphraseHelp")}
        </span>
      </div>
      <div id={feedbackId} className={styles.feedback}>
        {controller.accountAction ? (
          <FieldFeedback state="pending">
            {t(controller.accountAction === "sign-up"
              ? "settings.sync.cloud.signingUp"
              : "settings.sync.cloud.signingIn")}
          </FieldFeedback>
        ) : controller.accountError ? (
          <FieldFeedback state="error">
            {t(controller.accountError === "sign-up"
              ? "settings.sync.cloud.signUpError"
              : "settings.sync.cloud.signInError")}
          </FieldFeedback>
        ) : controller.accountDirty ? (
          <span className={styles.dirty}>{t("settings.sync.cloud.credentialsUnsaved")}</span>
        ) : null}
      </div>
      <div className={styles.formActions}>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={controller.busy || !controller.accountDirty}
          onClick={controller.clearAccount}
        >
          {t("settings.sync.cloud.discard")}
        </Button>
        <Button
          type="button"
          size="sm"
          variant="secondary"
          state={controller.accountAction === "sign-up" ? "loading" : "normal"}
          disabled={controller.busy || !controller.accountComplete}
          onClick={controller.submitSignUp}
        >
          {controller.accountAction !== "sign-up" ? (
            <Icon name="plus" aria-hidden="true" />
          ) : null}
          {t(controller.accountAction === "sign-up"
            ? "settings.sync.cloud.signingUp"
            : "settings.sync.cloud.createAccount")}
        </Button>
        <Button
          type="submit"
          size="sm"
          state={controller.accountAction === "sign-in" ? "loading" : "normal"}
          disabled={controller.busy || !controller.accountComplete}
        >
          {controller.accountAction !== "sign-in" ? (
            <Icon name="login" aria-hidden="true" />
          ) : null}
          {t(controller.accountAction === "sign-in"
            ? "settings.sync.cloud.signingIn"
            : "settings.sync.cloud.signIn")}
        </Button>
      </div>
    </form>
  );
}
