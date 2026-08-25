import { type ReactNode, useEffect, useId, useState } from "react";

import {
  BrandMark,
  MetadataLabel,
  MetadataList,
  MetadataRow,
  MetadataValue,
  SkeletonText,
} from "@/components/shared";
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
import { UpdateRow } from "@/features/settings/components/UpdateRow";
import { statusService, useStatus } from "@/hooks/useStatus";
import { useTranslation } from "@/i18n";
import { appVersion as readAppVersion } from "@/lib/appVersion";
import { classifyError, friendlyError } from "@/lib/errors";
import { CURRENT_PROTOCOL_VERSION } from "@/lib/ipc";
import {
  PRODUCT_RELEASES_URL,
  PRODUCT_REPOSITORY_URL,
} from "@/lib/productLinks";
import { usePrefs } from "@/store/prefs";
import styles from "./AboutTab.module.css";

const REAL_BACKENDS = /pasteboard|nspasteboard|system/i;
const LINKS = [
  { key: "repository", icon: "github", href: PRODUCT_REPOSITORY_URL },
  { key: "releases", icon: "fileText", href: PRODUCT_RELEASES_URL },
] as const satisfies ReadonlyArray<{
  key: "repository" | "releases";
  icon: IconName;
  href: string;
}>;

function RuntimeRow({ title, children }: { title: string; children: ReactNode }) {
  return (
    <MetadataRow data-settings-search-target={`row:${title}`}>
      <MetadataLabel>{title}</MetadataLabel>
      <MetadataValue>{children}</MetadataValue>
    </MetadataRow>
  );
}

export function AboutTab() {
  const { t } = useTranslation();
  const status = useStatus(statusService);
  const resetPrefs = usePrefs((state) => state.reset);
  const [version, setVersion] = useState(__COPYPASTE_APP_VERSION__);
  const [resetOpen, setResetOpen] = useState(false);
  const resetDescriptionId = useId();

  useEffect(() => {
    let active = true;
    void readAppVersion().then((value) => {
      if (active) setVersion(value);
    });
    return () => {
      active = false;
    };
  }, []);

  const backendIsReal = status.data
    ? REAL_BACKENDS.test(status.data.clipboard_backend)
    : true;
  const mismatch = status.data !== undefined &&
    status.data.protocol_version !== CURRENT_PROTOCOL_VERSION;
  const captureLabel = status.data
    ? t(
        status.data.capture_running
          ? "settings.about.capture.running"
          : "settings.about.capture.paused",
      )
    : "";

  return (
    <div className={styles.root}>
      <div className={styles.layout}>
      <div
        className={styles.identity}
        data-settings-search-target={`row:${t("settings.about.app.title")}`}
      >
        <BrandMark size="app" animated />
        <div className={styles.identityCopy}>
          <strong>CopyPaste</strong>
          <span>{t("settings.about.app.version", { version })}</span>
        </div>
        <span className={styles.tagline}>{t("settings.about.app.tagline")}</span>
      </div>

      <UpdateRow />

      <section className={styles.section} aria-labelledby="about-runtime-title">
        <h3 id="about-runtime-title" className={styles.sectionTitle}>
          {t("settings.about.runtime.title")}
        </h3>
        <MetadataList className={styles.runtimeList}>
          <RuntimeRow title={t("settings.about.service.title")}>
            {status.error ? (
              <span className={styles.error}>
                {friendlyError(classifyError(status.error))}
              </span>
            ) : status.data ? (
              t("settings.about.service.version", { version: status.data.version })
            ) : <SkeletonText width="sm" />}
          </RuntimeRow>

          <RuntimeRow title={t("settings.about.capture.title")}>
            {status.data ? (
              <Badge variant={status.data.capture_running ? "ok" : "warn"}>
                {captureLabel}
              </Badge>
            ) : <SkeletonText width="xs" />}
          </RuntimeRow>

          <RuntimeRow title={t("settings.about.backend.title")}>
            {status.data ? (
              <Badge variant={backendIsReal ? "secondary" : "warn"} className={styles.valueBadge}>
                {status.data.clipboard_backend}
              </Badge>
            ) : <SkeletonText width="sm" />}
          </RuntimeRow>

          <RuntimeRow title={t("settings.about.protocol.title")}>
            {status.data ? (
              <Badge variant={mismatch ? "error" : "secondary"} className={styles.valueBadge}>
                {t("settings.about.protocol.value", { version: status.data.protocol_version })}
                {mismatch
                  ? ` ${t("settings.about.protocol.mismatch", { version: CURRENT_PROTOCOL_VERSION })}`
                  : ""}
              </Badge>
            ) : <SkeletonText width="xs" />}
          </RuntimeRow>

          <RuntimeRow title={t("settings.about.items.title")}>
            {status.data ? (
              <span className={styles.numeric}>
                {status.data.item_count.toLocaleString()}
              </span>
            ) : <SkeletonText width="xs" />}
          </RuntimeRow>
        </MetadataList>
      </section>

      <section
        className={styles.section}
        aria-labelledby="about-resources-title"
        data-settings-search-target={`section:${t("settings.about.links.title")}`}
      >
        <h3 id="about-resources-title" className={styles.sectionTitle}>
          {t("settings.about.links.title")}
        </h3>
        <div className={styles.openList}>
          {LINKS.map(({ key, icon, href }) => (
            <Button key={key} asChild variant="ghost" size="md" className={styles.openRow}>
              <a href={href} target="_blank" rel="noreferrer">
                <span className={styles.openLayout}>
                  <Icon name={icon} size="md" className={styles.openIcon} />
                  <span className={styles.openCopy}>
                    <strong>{t(`settings.about.links.${key}`)}</strong>
                    <span>{t(`settings.about.links.${key}Description`)}</span>
                  </span>
                  <Icon name="caretRight" className={styles.openCaret} />
                </span>
              </a>
            </Button>
          ))}
        </div>
      </section>

      <section
        className={styles.resetSection}
        aria-labelledby="about-reset-title"
        data-settings-search-target={`row:${t("settings.about.reset.title")}`}
      >
        <div className={styles.resetCopy}>
          <h3 id="about-reset-title">{t("settings.about.reset.title")}</h3>
          <p id={resetDescriptionId}>{t("settings.about.reset.description")}</p>
        </div>
        <Button
          variant="secondary"
          tone="danger"
          size="sm"
          className={styles.resetAction}
          aria-describedby={resetDescriptionId}
          onClick={() => setResetOpen(true)}
        >
          {t("settings.about.reset.action")}
        </Button>
      </section>

      <AlertDialog open={resetOpen} onOpenChange={setResetOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("settings.about.reset.confirmTitle")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("settings.about.reset.confirmDescription")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              variant="danger"
              tone="danger"
              onClick={() => {
                resetPrefs();
                setResetOpen(false);
              }}
            >
              {t("settings.about.reset.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      </div>
    </div>
  );
}
