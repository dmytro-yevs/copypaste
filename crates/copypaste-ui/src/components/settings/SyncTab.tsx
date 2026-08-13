import { type FormEvent, useState } from "react";
import { Cloud, LogIn, LogOut, MonitorSmartphone, RefreshCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Row } from "@/components/settings/Row";
import { DeviceNameField } from "@/components/devices/DeviceNameField";
import {
  useCloudSignIn,
  useCloudSignOut,
  useCloudStatus,
  useCloudSyncNow,
} from "@/hooks/useCloud";
import { usePeers, useSyncNow } from "@/hooks/useDevices";
import { useTranslation } from "@/i18n";
import { isUnavailable } from "@/lib/errors";
import { useUi } from "@/store/ui";

export function SyncTab() {
  const { t } = useTranslation();
  const peers = usePeers();
  const sync = useSyncNow();
  const cloud = useCloudStatus();
  const cloudSignIn = useCloudSignIn();
  const cloudSignOut = useCloudSignOut();
  const cloudSync = useCloudSyncNow();
  const setView = useUi((s) => s.setView);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [passphrase, setPassphrase] = useState("");

  const unavailable = peers.error !== null && isUnavailable(peers.error);
  const count = peers.data?.length ?? 0;
  const cloudStatus = cloud.data;
  const cloudBusy = cloudSignIn.isPending || cloudSignOut.isPending || cloudSync.isPending;

  function submitCloudSignIn(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    cloudSignIn.mutate(
      { email: email.trim(), password, passphrase },
      {
        onSuccess: () => {
          setPassword("");
          setPassphrase("");
        },
      },
    );
  }

  return (
    <div className="flex flex-col">
      <Row
        title={t("devices.own.rename.label")}
        description={t("devices.own.rename.description")}
      >
        <DeviceNameField />
      </Row>
      <Row
        title={t("settings.sync.paired.title")}
        description={t("settings.sync.paired.description")}
      >
        <div className="flex items-center gap-s-2">
          <Badge
            variant={
              unavailable ? "secondary" : count > 0 ? "secondary" : "warn"
            }
          >
            {unavailable
              ? t("settings.sync.paired.unavailable")
              : count === 0
                ? t("settings.sync.paired.none")
                : t("settings.sync.paired.count", { n: count })}
          </Badge>
          <Button variant="outline" size="sm" onClick={() => setView("devices")}>
            <MonitorSmartphone aria-hidden="true" />
            {t("settings.sync.paired.manage")}
          </Button>
        </div>
      </Row>

      <Row
        title={t("settings.sync.now.title")}
        description={t("settings.sync.now.description")}
      >
        <Button
          variant="outline"
          size="sm"
          disabled={sync.isPending || count === 0 || unavailable}
          onClick={() => sync.mutate(undefined)}
        >
          <RefreshCw aria-hidden="true" />
          {t(sync.isPending ? "settings.sync.now.pending" : "settings.sync.now.action")}
        </Button>
      </Row>

      <Row
        title={t("settings.sync.cloud.title")}
        description={t(
          cloudStatus?.configured
            ? "settings.sync.cloud.description"
            : "settings.sync.cloud.notConfigured",
        )}
        badge={
          <Badge
            variant={
              cloud.isError || !cloudStatus?.configured
                ? "warn"
                : cloudStatus.signed_in && cloudStatus.key_ready
                  ? "ok"
                  : "secondary"
            }
          >
            {t(
              cloud.isError
                ? "settings.sync.cloud.badgeUnavailable"
                : !cloudStatus?.configured
                  ? "settings.sync.cloud.badgeNotConfigured"
                  : cloudStatus.signed_in && cloudStatus.key_ready
                    ? "settings.sync.cloud.badgeConnected"
                    : "settings.sync.cloud.badgeSignedOut",
            )}
          </Badge>
        }
        note={
          cloudStatus?.last_error ? (
            <span role="status" className="text-xs text-warn-strong">
              {t("settings.sync.cloud.lastError")}
            </span>
          ) : cloudStatus?.unreadable_uploads ? (
            // A separate line rather than an error badge: sync is working, and
            // these rows are being retried. Without it, "everything but these"
            // is indistinguishable from "everything".
            <span role="status" className="text-xs text-warn-strong">
              {t("settings.sync.cloud.unreadableUploads", {
                count: cloudStatus.unreadable_uploads,
              })}
            </span>
          ) : undefined
        }
      >
        {cloud.isLoading ? (
          <span role="status" className="text-sm text-muted-foreground">
            {t("settings.sync.cloud.loading")}
          </span>
        ) : cloud.isError ? (
          <Button variant="outline" size="sm" onClick={() => void cloud.refetch()}>
            <RefreshCw aria-hidden="true" />
            {t("settings.sync.cloud.retry")}
          </Button>
        ) : !cloudStatus?.configured ? (
          <Cloud aria-hidden="true" className="text-muted-foreground" />
        ) : cloudStatus.signed_in && cloudStatus.key_ready ? (
          <div className="flex max-w-[360px] flex-col items-end gap-s-2">
            {cloudStatus.email && (
              <span className="max-w-full truncate text-xs text-muted-foreground">
                {cloudStatus.email}
              </span>
            )}
            <div className="flex flex-wrap justify-end gap-s-2">
              <Button
                variant="outline"
                size="sm"
                disabled={cloudBusy}
                onClick={() => cloudSync.mutate()}
              >
                <RefreshCw aria-hidden="true" />
                {t(
                  cloudSync.isPending
                    ? "settings.sync.cloud.syncing"
                    : "settings.sync.cloud.syncNow",
                )}
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={cloudBusy}
                onClick={() => cloudSignOut.mutate()}
              >
                <LogOut aria-hidden="true" />
                {t("settings.sync.cloud.signOut")}
              </Button>
            </div>
          </div>
        ) : (
          <form
            className="grid w-full max-w-[360px] gap-s-2"
            aria-label={t("settings.sync.cloud.formLabel")}
            onSubmit={submitCloudSignIn}
          >
            <Input
              type="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
              placeholder={t("settings.sync.cloud.email")}
              aria-label={t("settings.sync.cloud.email")}
              autoComplete="username"
              disabled={cloudBusy}
              required
            />
            <Input
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              placeholder={t("settings.sync.cloud.password")}
              aria-label={t("settings.sync.cloud.password")}
              autoComplete="current-password"
              disabled={cloudBusy}
              required
            />
            <Input
              type="password"
              value={passphrase}
              onChange={(event) => setPassphrase(event.target.value)}
              placeholder={t("settings.sync.cloud.passphrase")}
              aria-label={t("settings.sync.cloud.passphrase")}
              autoComplete="off"
              disabled={cloudBusy}
              required
            />
            <Button
              type="submit"
              size="sm"
              disabled={cloudBusy || !email.trim() || !password || !passphrase}
            >
              <LogIn aria-hidden="true" />
              {t(
                cloudSignIn.isPending
                  ? "settings.sync.cloud.signingIn"
                  : "settings.sync.cloud.signIn",
              )}
            </Button>
          </form>
        )}
      </Row>
    </div>
  );
}
