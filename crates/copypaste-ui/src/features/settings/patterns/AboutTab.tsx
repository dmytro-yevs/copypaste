import { useEffect, useState } from "react";
import { BrandMark, SkeletonText } from "@/components/shared";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, Badge } from "@/components/ui";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { UpdateRow } from "@/features/settings/components/UpdateRow";
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
  const openOnboarding = useUi((state) => state.openOnboarding);
  const [version, setVersion] = useState(__COPYPASTE_APP_VERSION__);
  const [resetOpen, setResetOpen] = useState(false);
  useEffect(() => { let active = true; void readAppVersion().then((value) => { if (active) setVersion(value); }); return () => { active = false; }; }, []);

  const backendIsReal = status.data ? REAL_BACKENDS.test(status.data.clipboard_backend) : true;
  const mismatch = status.data !== undefined && status.data.protocol_version !== CURRENT_PROTOCOL_VERSION;
  const snapshot = capture.data;
  const desktopCapture = snapshot?.rung === "desktop";
  const capturePresentation = snapshot === undefined ? undefined : capturePresentationOf(snapshot.health);
  const captureVariant = desktopCapture ? status.data?.capture_running ? "ok" : "warn" : capturePresentation?.tone === "positive" ? "ok" : capturePresentation?.tone === "danger" ? "error" : capturePresentation?.tone === "attention" ? "warn" : "info";
  const captureLabel = desktopCapture ? t(status.data?.capture_running ? "settings.about.capture.running" : "settings.about.capture.paused") : snapshot?.headline;

  return <div className={styles.root}><div className={styles.layout}>
    <div className={styles.identity} data-settings-search-target={`row:${t("settings.about.app.title")}`}><BrandMark size="app" animated /><div className={styles.identityCopy}><strong>CopyPaste</strong><span>{t("settings.about.app.version", { version })}</span></div><span className={styles.tagline}>{t("settings.about.app.tagline")}</span></div>
    <UpdateRow />
    <SettingsSchemaRenderer groups={[{
      id: "runtime", title: t("settings.about.runtime.title"), fields: [
        { kind: "status", definition: settingDefinition("about", "settings.about.service.title"), value: status.error ? <span className={styles.error}>{friendlyError(classifyError(status.error))}</span> : status.data ? t("settings.about.service.version", { version: status.data.version }) : <SkeletonText width="sm" /> },
        { kind: "status", definition: settingDefinition("about", "settings.about.capture.title"), value: capture.isError || (desktopCapture && status.isError) ? <Badge variant="warn">{t("settings.about.capture.unavailable")}</Badge> : snapshot && (!desktopCapture || status.data) ? <Badge variant={captureVariant} className={styles.valueBadge}>{captureLabel}{!desktopCapture && snapshot.health.state !== "working" ? ` ${t("settings.about.capture.manualAvailable")}` : ""}</Badge> : <Badge variant="secondary" role="status" aria-label={t("settings.about.capture.loading")}>{t("settings.about.capture.loading")}</Badge> },
        { kind: "readonly", definition: settingDefinition("about", "settings.about.backend.title"), value: status.data ? <Badge variant={backendIsReal ? "secondary" : "warn"} className={styles.valueBadge}>{status.data.clipboard_backend}</Badge> : <SkeletonText width="sm" /> },
        { kind: "readonly", definition: settingDefinition("about", "settings.about.protocol.title"), value: status.data ? <Badge variant={mismatch ? "error" : "secondary"} className={styles.valueBadge}>{t("settings.about.protocol.value", { version: status.data.protocol_version })}{mismatch ? ` ${t("settings.about.protocol.mismatch", { version: CURRENT_PROTOCOL_VERSION })}` : ""}</Badge> : <SkeletonText width="xs" /> },
        { kind: "readonly", definition: settingDefinition("about", "settings.about.items.title"), value: status.data ? <span className={styles.numeric}>{status.data.item_count.toLocaleString()}</span> : <SkeletonText width="xs" /> },
      ],
    }, {
      id: "links", title: t("settings.about.links.title"), fields: [
        { kind: "action", definition: settingDefinition("about", "settings.about.links.repository"), label: t("settings.about.links.repository"), href: PRODUCT_REPOSITORY_URL },
        { kind: "action", definition: settingDefinition("about", "settings.about.links.releases"), label: t("settings.about.links.releases"), href: PRODUCT_RELEASES_URL },
      ],
    }, {
      id: "welcome", fields: [{ kind: "action", definition: settingDefinition("about", "onboarding.settings.title"), label: t("onboarding.settings.action"), onAction: openOnboarding }],
    }, {
      id: "reset", fields: [{ kind: "action", definition: settingDefinition("about", "settings.about.reset.title"), label: t("settings.about.reset.action"), tone: "danger", onAction: () => setResetOpen(true) }],
    }]} />
    <AlertDialog open={resetOpen} onOpenChange={setResetOpen}><AlertDialogContent><AlertDialogHeader><AlertDialogTitle>{t("settings.about.reset.confirmTitle")}</AlertDialogTitle><AlertDialogDescription>{t("settings.about.reset.confirmDescription")}</AlertDialogDescription></AlertDialogHeader><AlertDialogFooter><AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel><AlertDialogAction variant="danger" tone="danger" onClick={() => { resetPrefs(); setResetOpen(false); }}>{t("settings.about.reset.action")}</AlertDialogAction></AlertDialogFooter></AlertDialogContent></AlertDialog>
  </div></div>;
}
