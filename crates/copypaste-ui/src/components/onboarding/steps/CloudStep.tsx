import { type FormEvent, useState } from "react";
import { Cloud } from "lucide-react";

import { OnboardingStepLayout } from "@/components/onboarding/OnboardingStepLayout";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  useCloudSetEndpoint,
  useCloudSignIn,
  useCloudSignUp,
  useCloudStatus,
} from "@/hooks/useCloud";
import { useTranslation } from "@/i18n";

export function CloudStep({
  continue: onContinue,
  skip,
  optional,
}: OnboardingStepProps) {
  const { t } = useTranslation();
  const cloud = useCloudStatus();
  const signIn = useCloudSignIn();
  const signUp = useCloudSignUp();
  const setEndpoint = useCloudSetEndpoint();
  const status = cloud.data;
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [url, setUrl] = useState("");
  const [anonKey, setAnonKey] = useState("");
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const busy = signIn.isPending || signUp.isPending || setEndpoint.isPending;
  const ready = Boolean(status?.configured);
  const connected = Boolean(status?.signed_in && status.key_ready);

  function credentials() {
    return { email: email.trim(), password, passphrase };
  }

  function submitSignIn(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    signIn.mutate(credentials(), { onSuccess: () => onContinue() });
  }

  return (
    <OnboardingStepLayout
      icon={Cloud}
      title={t("onboarding.cloud.title")}
      body={
        connected
          ? t("onboarding.cloud.connected")
          : ready
            ? t("onboarding.cloud.hosted")
            : t("onboarding.cloud.body")
      }
      primary={{ label: t("onboarding.cloud.action"), onClick: onContinue }}
      skip={
        optional ? { label: t("onboarding.skip"), onClick: skip } : undefined
      }
    >
      {connected ? null : (
        <div className="flex flex-col gap-s-3">
          {ready ? (
            <form
              className="grid gap-s-2"
              aria-label={t("settings.sync.cloud.formLabel")}
              onSubmit={submitSignIn}
            >
              <Input
                type="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                placeholder={t("settings.sync.cloud.email")}
                aria-label={t("settings.sync.cloud.email")}
                autoComplete="username"
                disabled={busy}
                required
              />
              <Input
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                placeholder={t("settings.sync.cloud.password")}
                aria-label={t("settings.sync.cloud.password")}
                autoComplete="new-password"
                disabled={busy}
                required
              />
              <Input
                type="password"
                value={passphrase}
                onChange={(event) => setPassphrase(event.target.value)}
                placeholder={t("settings.sync.cloud.passphrase")}
                aria-label={t("settings.sync.cloud.passphrase")}
                autoComplete="off"
                disabled={busy}
                required
              />
              <div className="flex flex-wrap gap-s-2">
                <Button
                  type="submit"
                  size="sm"
                  disabled={busy || !email.trim() || !password || !passphrase}
                >
                  {t(
                    signIn.isPending
                      ? "settings.sync.cloud.signingIn"
                      : "onboarding.cloud.signIn",
                  )}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  disabled={busy || !email.trim() || !password || !passphrase}
                  onClick={() =>
                    signUp.mutate(credentials(), { onSuccess: () => onContinue() })
                  }
                >
                  {t("onboarding.cloud.createAccount")}
                </Button>
              </div>
            </form>
          ) : null}

          <details
            className="rounded-lg border border-border bg-card px-s-3 py-s-2"
            onToggle={(event) =>
              setAdvancedOpen((event.currentTarget as HTMLDetailsElement).open)
            }
          >
            <summary className="cursor-pointer text-sm font-medium">
              {t("onboarding.cloud.advanced")}
            </summary>
            {advancedOpen ? (
            <form
              className="mt-s-2 grid gap-s-2"
              aria-label={t("onboarding.cloud.advancedForm")}
              onSubmit={(event) => {
                event.preventDefault();
                setEndpoint.mutate({ url, anonKey });
              }}
            >
              <Input
                value={url}
                onChange={(event) => setUrl(event.target.value)}
                placeholder={t("onboarding.cloud.url")}
                aria-label={t("onboarding.cloud.url")}
                disabled={busy}
              />
              <Input
                value={anonKey}
                onChange={(event) => setAnonKey(event.target.value)}
                placeholder={t("onboarding.cloud.anonKey")}
                aria-label={t("onboarding.cloud.anonKey")}
                autoComplete="off"
                disabled={busy}
              />
              <Button type="submit" size="sm" variant="outline" disabled={busy}>
                {t("onboarding.cloud.saveEndpoint")}
              </Button>
            </form>
            ) : null}
          </details>
        </div>
      )}
    </OnboardingStepLayout>
  );
}
