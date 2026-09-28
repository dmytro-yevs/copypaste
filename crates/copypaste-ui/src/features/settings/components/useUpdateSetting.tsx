import { useEffect, useState, type ReactNode } from "react";

import { StateView } from "@/components/shared/StateView";
import { AlertDialog, Badge } from "@/components/ui";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import type { SettingsField } from "@/features/settings/model/settingsFieldSchema";
import { useTranslation } from "@/i18n";
import { friendlyError, ipcFailure, type ErrorKind } from "@/lib/errors";
import { currentPlatform } from "@/lib/platform";
import { PRODUCT_RELEASES_URL } from "@/lib/productLinks";
import {
  checkForUpdate,
  getUpdateStatus,
  installUpdate,
  type UpdateProgress,
  type UpdateStatus,
} from "@/lib/updater";
import styles from "./useUpdateSetting.module.css";

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

/** Owns updater effects and maps their state into the shared settings renderer. */
export function useUpdateSetting(): { field: SettingsField; dialog: ReactNode } {
  const { t } = useTranslation();
  const platform = currentPlatform();
  const [state, setState] = useState<ViewState>({ state: "loading" });
  const [confirmingVersion, setConfirmingVersion] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void getUpdateStatus().then(
      (status) => { if (active) setState(status); },
      (error) => { if (active) setState(updateError(error)); },
    );
    return () => { active = false; };
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
    setState(platform === "macos" ? { state: "installing" } : { state: "preparing" });
    const onProgress = (progress: UpdateProgress) => {
      if (progress.state === "downloading") setState({ ...progress, version });
      else setState({ state: progress.state });
    };
    try {
      setState(await installUpdate(version, onProgress));
    } catch (error) {
      setState(updateError(error, version));
    }
  };

  const percent = state.state === "downloading"
    ? progressPercent(state.downloaded, state.total)
    : undefined;
  const availableVersion = state.state === "available" || state.state === "declined"
    ? state.version
    : undefined;
  const permissionVersion = state.state === "error" && state.kind === "update_permission_required"
    ? state.version
    : undefined;
  const description = state.state === "unsupported"
    ? t("settings.about.updates.descriptionUnsupported")
    : platform === "macos"
      ? t(state.state === "unconfigured"
        ? "settings.about.updates.descriptionMacosManual"
        : "settings.about.updates.descriptionMacos")
      : platform === "windows"
        ? t("settings.about.updates.descriptionWindows")
        : platform === "android"
          ? t("settings.about.updates.descriptionAndroid")
          : t("settings.about.updates.description");

  let message: string;
  switch (state.state) {
    case "unsupported": message = t("settings.about.updates.unsupported"); break;
    case "unconfigured": message = t(platform === "macos"
      ? "settings.about.updates.unconfiguredMacos"
      : "settings.about.updates.unconfigured"); break;
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

  const definition = settingDefinition("about", "settings.about.updates.title");
  const busy = state.state === "loading" || state.state === "checking" ||
    state.state === "preparing" || state.state === "downloading" ||
    state.state === "verifying" || state.state === "installing";
  const note = state.state === "checking"
    ? <StateView mode="loading" placement="inline" title={message} />
    : <span role={state.state === "error" ? "alert" : "status"} aria-live={state.state === "error" ? "assertive" : "polite"}>{message}</span>;
  const base = { definition, help: description, note };
  let field: SettingsField;

  if (state.state === "ready" || state.state === "checking" || state.state === "up_to_date") {
    field = {
      ...base, kind: "action", busy: state.state === "checking", disabled: state.state === "checking",
      label: t(state.state === "up_to_date" ? "settings.about.updates.checkAgain" : "settings.about.updates.check"),
      onAction: () => { void check(); },
    };
  } else if (availableVersion !== undefined) {
    field = { ...base, kind: "action", label: t("settings.about.updates.install"), onAction: () => setConfirmingVersion(availableVersion) };
  } else if (state.state === "downloading") {
    field = {
      ...base, kind: "status", busy: true,
      value: <progress aria-label={t("settings.about.updates.downloadProgress", { version: state.version })} max={100} value={percent} className={styles.progress} />,
    };
  } else if (permissionVersion !== undefined) {
    field = { ...base, kind: "action", label: t("settings.about.updates.continue"), onAction: () => { void install(permissionVersion); } };
  } else if (state.state === "error" && state.retryable) {
    field = { ...base, kind: "action", label: t("common.tryAgain"), onAction: () => { void (state.version ? install(state.version) : check()); } };
  } else if (state.state === "error") {
    field = { ...base, kind: "status", value: <Badge variant="error">{t("settings.about.updates.attentionLabel")}</Badge> };
  } else if (state.state === "unsupported") {
    field = { ...base, kind: "status", value: <Badge variant="secondary">{t("settings.about.updates.unavailableLabel")}</Badge> };
  } else if (state.state === "unconfigured") {
    field = platform === "macos"
      ? { ...base, kind: "action", label: t("settings.about.updates.viewReleases"), href: PRODUCT_RELEASES_URL }
      : { ...base, kind: "status", value: <Badge variant="warn">{t("settings.about.updates.unconfiguredLabel")}</Badge> };
  } else {
    field = { definition, help: description, kind: "status", busy, value: <StateView mode="loading" placement="inline" title={message} /> };
  }

  const confirmationDescription = platform === "macos"
    ? t("settings.about.updates.confirmDescriptionMacos")
    : platform === "android"
      ? t("settings.about.updates.confirmDescriptionAndroid")
      : t("settings.about.updates.confirmDescriptionWindows");
  const dialog = <AlertDialog
    open={confirmingVersion !== null}
    onOpenChange={(open) => { if (!open) setConfirmingVersion(null); }}
    title={t("settings.about.updates.confirmTitle", { version: confirmingVersion ?? "" })}
    description={confirmationDescription}
    cancel={{
      label: t("settings.about.updates.later"),
      onClick: () => { if (confirmingVersion) setState({ state: "declined", version: confirmingVersion }); },
    }}
    action={{
      label: t(platform === "macos" ? "settings.about.updates.updateAndRestart" : "settings.about.updates.installAndRestart"),
      onClick: () => { if (confirmingVersion) void install(confirmingVersion); },
    }}
  />;

  return { field, dialog };
}
