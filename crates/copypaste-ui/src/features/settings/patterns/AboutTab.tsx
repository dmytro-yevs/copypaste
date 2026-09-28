import { useEffect, useState } from "react";
import { BrandMark } from "@/components/shared";
import { StateView, type StateMode } from "@/components/shared/StateView";
import { AlertDialog, Badge } from "@/components/ui";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { settingsGroups } from "@/features/settings/model/settingsProjection";
import type { SettingsField } from "@/features/settings/model/settingsFieldSchema";
import { useUpdateSetting } from "@/features/settings/components/useUpdateSetting";
import { capturePresentationOf } from "@/features/capture/model";
import { useCaptureState } from "@/hooks/useCapture";
import { statusService, useStatus } from "@/hooks/useStatus";
import { useTranslation } from "@/i18n";
import { appVersion as readAppVersion } from "@/lib/appVersion";
import { classifyError, friendlyError } from "@/lib/errors";
import { CURRENT_PROTOCOL_VERSION } from "@/lib/ipc";
import { PRODUCT_RELEASES_URL, PRODUCT_REPOSITORY_URL } from "@/lib/productLinks";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";
import styles from "./AboutTab.module.css";

const REAL_BACKENDS = /pasteboard|nspasteboard|system/i;

export function AboutTab() {
  const { t } = useTranslation();
  const status = useStatus(statusService);
  const capture = useCaptureState();
  const resetPrefs = usePrefs((state) => state.reset);
  const update = useUpdateSetting();
  const openOnboarding = useUi((state) => state.openOnboarding);
  const [version, setVersion] = useState(__COPYPASTE_APP_VERSION__);
  const [resetOpen, setResetOpen] = useState(false);
  useEffect(() => { let active = true; void readAppVersion().then((value) => { if (active) setVersion(value); }); return () => { active = false; }; }, []);

  const backendIsReal = status.data ? REAL_BACKENDS.test(status.data.clipboard_backend) : true;
  const mismatch = status.data !== undefined && status.data.protocol_version !== CURRENT_PROTOCOL_VERSION;
  const snapshot = capture.data;
  const desktopCapture = snapshot?.rung === "desktop";
  const capturePresentation = snapshot === undefined ? undefined : capturePresentationOf(snapshot.health);
  const captureMode: StateMode = desktopCapture ? status.data?.capture_running ? "success" : "warning" : capturePresentation?.tone === "positive" ? "success" : capturePresentation?.tone === "danger" ? "error" : capturePresentation?.tone === "attention" ? "warning" : "info";
  const captureLabel = desktopCapture ? t(status.data?.capture_running ? "settings.about.capture.running" : "settings.about.capture.paused") : snapshot?.headline;

  const fields: SettingsField[] = [
    { kind: "custom", definition: settingDefinition("about", "settings.about.app.title"), rowless: true, content: <div className={styles.identity}><BrandMark size="app" animated /><div className={styles.identityCopy}><strong>CopyPaste</strong><span>{t("settings.about.app.version", { version })}</span></div><span className={styles.tagline}>{t("settings.about.app.tagline")}</span></div> },
    update.field,
    { kind: "status", definition: settingDefinition("about", "settings.about.service.title"), value: status.error ? <StateView mode="error" placement="control" title={friendlyError(classifyError(status.error))} /> : status.data ? t("settings.about.service.version", { version: status.data.version }) : <StateView mode="loading" placement="control" title="Checking…" /> },
    { kind: "status", definition: settingDefinition("about", "settings.about.capture.title"), value: capture.isError || (desktopCapture && status.isError) ? <StateView mode="warning" placement="control" title={t("settings.about.capture.unavailable")} /> : snapshot && (!desktopCapture || status.data) ? <StateView mode={captureMode} placement="control" title={<>{captureLabel}{!desktopCapture && snapshot.health.state !== "working" ? ` ${t("settings.about.capture.manualAvailable")}` : ""}</>} /> : <StateView mode="loading" placement="control" title={t("settings.about.capture.loading")} aria-label={t("settings.about.capture.loading")} /> },
    { kind: "readonly", definition: settingDefinition("about", "settings.about.backend.title"), value: status.data ? <Badge variant={backendIsReal ? "secondary" : "warn"} className={styles.valueBadge}>{status.data.clipboard_backend}</Badge> : <StateView mode="loading" placement="control" title="Checking…" /> },
    { kind: "readonly", definition: settingDefinition("about", "settings.about.protocol.title"), value: status.data ? <Badge variant={mismatch ? "error" : "secondary"} className={styles.valueBadge}>{t("settings.about.protocol.value", { version: status.data.protocol_version })}{mismatch ? ` ${t("settings.about.protocol.mismatch", { version: CURRENT_PROTOCOL_VERSION })}` : ""}</Badge> : <StateView mode="loading" placement="control" title="Checking…" /> },
    { kind: "readonly", definition: settingDefinition("about", "settings.about.items.title"), value: status.data ? <span className={styles.numeric}>{status.data.item_count.toLocaleString()}</span> : <StateView mode="loading" placement="control" title="Checking…" /> },
    { kind: "action", definition: settingDefinition("about", "settings.about.links.repository"), label: t("settings.about.links.repository"), href: PRODUCT_REPOSITORY_URL },
    { kind: "action", definition: settingDefinition("about", "settings.about.links.releases"), label: t("settings.about.links.releases"), href: PRODUCT_RELEASES_URL },
    { kind: "action", definition: settingDefinition("about", "onboarding.settings.title"), label: t("onboarding.settings.action"), onAction: openOnboarding },
    { kind: "action", definition: settingDefinition("about", "settings.about.reset.title"), label: t("settings.about.reset.action"), tone: "danger", onAction: () => setResetOpen(true) },
  ];

  return <div className={styles.root}><div className={styles.layout}>
    <SettingsSchemaRenderer groups={settingsGroups("about", fields, (key) => t(key as never))} />
    {update.dialog}
    <AlertDialog open={resetOpen} onOpenChange={setResetOpen} title={t("settings.about.reset.confirmTitle")} description={t("settings.about.reset.confirmDescription")} cancel={{ label: t("common.cancel") }} action={{ label: t("settings.about.reset.action"), variant: "danger", tone: "danger", onClick: () => { resetPrefs(); setResetOpen(false); } }} />
  </div></div>;
}
