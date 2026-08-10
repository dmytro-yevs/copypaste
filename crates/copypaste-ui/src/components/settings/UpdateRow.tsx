import { useEffect, useState } from "react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Row } from "@/components/settings/Row";
import { useTranslation } from "@/i18n";
import { friendlyError, ipcFailure, type ErrorKind } from "@/lib/errors";
import {
  checkForUpdate,
  getUpdateStatus,
  installUpdate,
  type UpdateProgress,
  type UpdateStatus,
} from "@/lib/updater";

type ViewState =
  | UpdateStatus
  | { state: "loading" | "checking" | "verifying" | "installing" }
  | { state: "downloading"; downloaded: number; total: number | null; version: string }
  | { state: "declined"; version: string }
  | { state: "error"; kind: ErrorKind; retryable: boolean; version?: string };

function updateError(raw: unknown, version?: string): ViewState {
  const failure = ipcFailure(raw);
  return { state: "error", kind: failure.kind, retryable: failure.retryable, version };
}

function progressPercent(downloaded: number, total: number | null): number | undefined {
  if (total === null || total <= 0) return undefined;
  return Math.min(100, Math.round((downloaded / total) * 100));
}

export function UpdateRow() {
  const { t } = useTranslation();
  const [state, setState] = useState<ViewState>({ state: "loading" });
  const [confirmingVersion, setConfirmingVersion] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void getUpdateStatus().then(
      (status) => active && setState(status),
      (error) => active && setState(updateError(error)),
    );
    return () => {
      active = false;
    };
  }, []);

  const check = async () => {
    setState({ state: "checking" });
    try {
      setState(await checkForUpdate());
    } catch (error) {
      setState(updateError(error));
    }
  };

  const install = async (version: string) => {
    setConfirmingVersion(null);
    setState({ state: "downloading", downloaded: 0, total: null, version });
    const onProgress = (progress: UpdateProgress) => {
      if (progress.state === "downloading") {
        setState({ ...progress, version });
      } else {
        setState({ state: progress.state });
      }
    };
    try {
      setState(await installUpdate(version, onProgress));
    } catch (error) {
      setState(updateError(error, version));
    }
  };

  const statusRole = state.state === "error" ? "alert" : "status";
  const percent = state.state === "downloading"
    ? progressPercent(state.downloaded, state.total)
    : undefined;
  const availableVersion = state.state === "available" || state.state === "declined"
    ? state.version
    : undefined;

  let message: string;
  switch (state.state) {
    case "loading": message = t("settings.about.updates.loading"); break;
    case "unsupported": message = t("settings.about.updates.unsupported"); break;
    case "unconfigured": message = t("settings.about.updates.unconfigured"); break;
    case "ready": message = t("settings.about.updates.ready"); break;
    case "checking": message = t("settings.about.updates.checking"); break;
    case "up_to_date": message = t("settings.about.updates.upToDate"); break;
    case "available": message = t("settings.about.updates.available", { version: state.version }); break;
    case "downloading": message = percent === undefined
      ? t("settings.about.updates.downloading")
      : t("settings.about.updates.downloadingPercent", { percent }); break;
    case "verifying": message = t("settings.about.updates.verifying"); break;
    case "installing": message = t("settings.about.updates.installing"); break;
    case "declined": message = t("settings.about.updates.declined", { version: state.version }); break;
    case "error": message = state.kind === "unknown"
      ? t("settings.about.updates.error")
      : friendlyError(state.kind); break;
  }

  return (
    <>
      <Row
        title={t("settings.about.updates.title")}
        description={t("settings.about.updates.description")}
        note={(
          <span
            role={statusRole}
            aria-live={statusRole === "alert" ? "assertive" : "polite"}
            className={state.state === "error" ? "text-xs text-err-strong" : "text-xs text-muted-foreground"}
          >
            {message}
          </span>
        )}
      >
        {state.state === "ready" || state.state === "up_to_date" ? (
          <Button variant="outline" size="sm" onClick={() => void check()}>
            {t(state.state === "ready" ? "settings.about.updates.check" : "settings.about.updates.checkAgain")}
          </Button>
        ) : availableVersion !== undefined ? (
          <Button size="sm" onClick={() => setConfirmingVersion(availableVersion)}>
            {t("settings.about.updates.install")}
          </Button>
        ) : state.state === "downloading" ? (
          <progress
            aria-label={t("settings.about.updates.downloadProgress", { version: state.version })}
            max={100}
            value={percent}
            className="w-36 accent-primary"
          />
        ) : state.state === "error" && state.retryable ? (
          <Button
            variant="outline"
            size="sm"
            onClick={() => state.version ? void install(state.version) : void check()}
          >
            {t("common.tryAgain")}
          </Button>
        ) : null}
      </Row>

      <AlertDialog
        open={confirmingVersion !== null}
        onOpenChange={(open) => !open && setConfirmingVersion(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("settings.about.updates.confirmTitle", { version: confirmingVersion ?? "" })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("settings.about.updates.confirmDescription")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel
              onClick={() => confirmingVersion && setState({ state: "declined", version: confirmingVersion })}
            >
              {t("settings.about.updates.later")}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={() => confirmingVersion && void install(confirmingVersion)}
            >
              {t("settings.about.updates.installAndRestart")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
