import { type ReactNode, useEffect, useId, useState } from "react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Badge,
  Button,
} from "@/components/ui";
import { Icon, type IconName } from "@/components/ui/icon";
import { useTranslation } from "@/i18n";
import { friendlyError, ipcFailure, type ErrorKind } from "@/lib/errors";
import { currentPlatform } from "@/lib/platform";
import {
  checkForUpdate,
  getUpdateStatus,
  installUpdate,
  type UpdateProgress,
  type UpdateStatus,
} from "@/lib/updater";
import styles from "./UpdateRow.module.css";

type ViewState =
  | UpdateStatus
  | { state: "loading" | "checking" | "preparing" | "verifying" | "installing" }
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

function stateIcon(state: ViewState): IconName {
  switch (state.state) {
    case "up_to_date": return "checkCircle";
    case "available":
    case "declined":
    case "downloading": return "download";
    case "error":
    case "unsupported":
    case "unconfigured": return "alert";
    case "loading":
    case "checking":
    case "preparing":
    case "verifying":
    case "installing": return "spinner";
    case "ready": return "refresh";
  }
}

export function UpdateRow() {
  const { t } = useTranslation();
  const platform = currentPlatform();
  const [state, setState] = useState<ViewState>({ state: "loading" });
  const [confirmingVersion, setConfirmingVersion] = useState<string | null>(null);
  const descriptionId = useId();
  const statusId = useId();

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
    setState(
      platform === "macos"
        ? { state: "installing" }
        : { state: "preparing" },
    );
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
  const permissionVersion = state.state === "error" &&
      state.kind === "update_permission_required"
    ? state.version
    : undefined;
  if (state.state === "unsupported" || state.state === "unconfigured") {
    return null;
  }

  const description = platform === "macos"
      ? t("settings.about.updates.descriptionMacos")
      : platform === "windows"
        ? t("settings.about.updates.descriptionWindows")
        : platform === "android"
          ? t("settings.about.updates.descriptionAndroid")
          : t("settings.about.updates.description");

  let message: string;
  switch (state.state) {
    case "loading": message = t("settings.about.updates.loading"); break;
    case "ready": message = t("settings.about.updates.ready"); break;
    case "checking": message = t("settings.about.updates.checking"); break;
    case "preparing": message = t("settings.about.updates.preparing"); break;
    case "up_to_date": message = t("settings.about.updates.upToDate"); break;
    case "available": message = t("settings.about.updates.available", { version: state.version }); break;
    case "downloading": message = percent === undefined
      ? t("settings.about.updates.downloading")
      : t("settings.about.updates.downloadingPercent", { percent }); break;
    case "verifying": message = t("settings.about.updates.verifying"); break;
    case "installing": message = platform === "macos"
      ? t("settings.about.updates.installingMacos")
      : t("settings.about.updates.installing"); break;
    case "declined": message = t("settings.about.updates.declined", { version: state.version }); break;
    case "error": message = state.kind === "unknown"
      ? t("settings.about.updates.error")
      : friendlyError(state.kind); break;
  }

  let action: ReactNode;
  if (state.state === "ready" || state.state === "up_to_date") {
    action = (
      <Button
        variant="secondary"
        size="sm"
        aria-describedby={`${descriptionId} ${statusId}`}
        onClick={() => void check()}
      >
        {t(state.state === "ready" ? "settings.about.updates.check" : "settings.about.updates.checkAgain")}
      </Button>
    );
  } else if (availableVersion !== undefined) {
    action = (
      <Button
        size="sm"
        aria-describedby={`${descriptionId} ${statusId}`}
        onClick={() => setConfirmingVersion(availableVersion)}
      >
        {t("settings.about.updates.install")}
      </Button>
    );
  } else if (state.state === "downloading") {
    action = (
      <progress
        aria-label={t("settings.about.updates.downloadProgress", { version: state.version })}
        max={100}
        value={percent}
        aria-describedby={`${descriptionId} ${statusId}`}
        className={styles.progress}
      />
    );
  } else if (permissionVersion !== undefined) {
    action = (
      <Button
        size="sm"
        aria-describedby={`${descriptionId} ${statusId}`}
        onClick={() => void install(permissionVersion)}
      >
        {t("settings.about.updates.continue")}
      </Button>
    );
  } else if (state.state === "error" && state.retryable) {
    action = (
      <Button
        variant="secondary"
        size="sm"
        aria-describedby={`${descriptionId} ${statusId}`}
        onClick={() => state.version ? void install(state.version) : void check()}
      >
        {t("common.tryAgain")}
      </Button>
    );
  } else if (state.state === "error") {
    action = <Badge variant="error">{t("settings.about.updates.attentionLabel")}</Badge>;
  } else {
    action = (
      <span className={styles.activity} aria-hidden="true">
        <Icon name="spinner" />
      </span>
    );
  }

  const confirmationDescription = platform === "macos"
    ? t("settings.about.updates.confirmDescriptionMacos")
    : platform === "android"
      ? t("settings.about.updates.confirmDescriptionAndroid")
      : t("settings.about.updates.confirmDescriptionWindows");

  return (
    <>
      <section
        className={styles.root}
        data-state={state.state}
        data-settings-search-target={`row:${t("settings.about.updates.title")}`}
        aria-labelledby="about-updates-title"
      >
        <span className={styles.stateIcon} aria-hidden="true">
          <Icon
            name={stateIcon(state)}
            size="md"
            className={state.state === "loading" || state.state === "checking" ||
                state.state === "preparing" || state.state === "verifying" ||
                state.state === "installing"
              ? styles.spinning
              : undefined}
          />
        </span>
        <div className={styles.copy}>
          <h3 id="about-updates-title">{t("settings.about.updates.title")}</h3>
          <p id={descriptionId}>{description}</p>
          <span
            key={`${state.state}-${message}`}
            id={statusId}
            role={statusRole}
            aria-live={statusRole === "alert" ? "assertive" : "polite"}
            className={state.state === "error" ? styles.errorStatus : styles.status}
          >
            {message}
          </span>
        </div>
        <div className={styles.action}>{action}</div>
      </section>

      <AlertDialog
        open={confirmingVersion !== null}
        onOpenChange={(open) => !open && setConfirmingVersion(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("settings.about.updates.confirmTitle", { version: confirmingVersion ?? "" })}
            </AlertDialogTitle>
            <AlertDialogDescription>{confirmationDescription}</AlertDialogDescription>
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
              {t(
                platform === "macos"
                  ? "settings.about.updates.updateAndRestart"
                  : "settings.about.updates.installAndRestart",
              )}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
