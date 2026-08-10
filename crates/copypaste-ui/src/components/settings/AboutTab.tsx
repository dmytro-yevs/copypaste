/**
 * `clipboard_backend` is surfaced because a fake backend must not be mistaken
 * for the real pasteboard: a build reading `fake` or `android-inprocess` says
 * so, in the warning colour, rather than looking like a working clipboard.
 *
 * Nothing here renders a path (INV-12), which is why the service is described
 * by version and backend rather than by where it lives.
 */
import { useQuery } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { ExternalLink, RotateCcw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useStatus } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import { classifyError, friendlyError } from "@/lib/errors";
import { CURRENT_PROTOCOL_VERSION, hasBridge } from "@/lib/ipc";
import { usePrefs } from "@/store/prefs";
import { Row } from "@/components/settings/Row";
import { UpdateRow } from "@/components/settings/UpdateRow";
import { isWindows } from "@/lib/platform";

const REAL_BACKENDS = /pasteboard|nspasteboard|system/i;

const REPO = "https://github.com/dmytro-yevs/copypaste";

/**
 * Release notes, not a changelog file: nothing in the tree generates one, and
 * `release.yml` publishes notes to that page on every tag.
 *
 * `tauri-plugin-opener` handles the `_blank` targets through the platform's
 * default browser, so the WebView is never replaced by a web page.
 */
const LINKS = [
  { key: "repository", href: REPO },
  { key: "releases", href: `${REPO}/releases` },
] as const;

export function AboutTab() {
  const { t } = useTranslation();
  const status = useStatus();
  const resetPrefs = usePrefs((s) => s.reset);

  const appVersion = useQuery({
    queryKey: ["app-version"],
    queryFn: getVersion,
    enabled: hasBridge(),
    staleTime: Infinity,
    retry: false,
  });

  const backendIsReal = status.data
    ? REAL_BACKENDS.test(status.data.clipboard_backend)
    : true;
  const mismatch =
    status.data !== undefined &&
    status.data.protocol_version !== CURRENT_PROTOCOL_VERSION;

  return (
    <div className="flex flex-col">
      <Row
        title={t("settings.about.app.title")}
        description={t("settings.about.app.description")}
      >
        <span className="text-sm tabular-nums text-muted-foreground">
          {appVersion.data === undefined
            ? t("common.unknown")
            : t("settings.about.app.version", { version: appVersion.data })}
        </span>
      </Row>

      {isWindows() && <UpdateRow />}

      <Row title={t("settings.about.service.title")}>
        {status.error ? (
          <span className="text-sm text-err-strong">
            {friendlyError(classifyError(status.error))}
          </span>
        ) : status.data ? (
          <span className="text-sm tabular-nums text-muted-foreground">
            {t("settings.about.service.version", { version: status.data.version })}
          </span>
        ) : (
          <span className="text-sm text-muted-foreground">
            {t("settings.about.service.connecting")}
          </span>
        )}
      </Row>

      <Row
        title={t("settings.about.capture.title")}
        description={t("settings.about.capture.description")}
      >
        {status.data ? (
          <Badge variant={status.data.capture_running ? "ok" : "warn"}>
            {t(
              status.data.capture_running
                ? "settings.about.capture.running"
                : "settings.about.capture.paused",
            )}
          </Badge>
        ) : (
          <Badge variant="secondary">{t("common.unknown")}</Badge>
        )}
      </Row>

      <Row
        title={t("settings.about.backend.title")}
        description={t("settings.about.backend.description")}
      >
        {status.data ? (
          <Badge variant={backendIsReal ? "secondary" : "warn"}>
            {status.data.clipboard_backend}
          </Badge>
        ) : (
          <Badge variant="secondary">{t("common.unknown")}</Badge>
        )}
      </Row>

      <Row
        title={t("settings.about.protocol.title")}
        description={t("settings.about.protocol.description")}
      >
        <Badge variant={mismatch ? "error" : "secondary"}>
          {t("settings.about.protocol.value", {
            version: status.data?.protocol_version ?? CURRENT_PROTOCOL_VERSION,
          })}
          {mismatch
            ? ` ${t("settings.about.protocol.mismatch", {
                version: CURRENT_PROTOCOL_VERSION,
              })}`
            : ""}
        </Badge>
      </Row>

      <Row
        title={t("settings.about.items.title")}
        description={t("settings.about.items.description")}
      >
        <span className="text-sm tabular-nums text-muted-foreground">
          {status.data
            ? status.data.item_count.toLocaleString()
            : t("common.noValue")}
        </span>
      </Row>

      <Row title={t("settings.about.links.title")}>
        <div className="flex flex-wrap items-center gap-s-3">
          {LINKS.map(({ key, href }) => (
            <a
              key={key}
              href={href}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1 rounded-sm text-sm text-muted-foreground underline underline-offset-2 hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring focus-visible:outline-none"
            >
              {t(`settings.about.links.${key}`)}
              <ExternalLink size={12} aria-hidden="true" />
            </a>
          ))}
        </div>
      </Row>

      <Row
        title={t("settings.about.reset.title")}
        description={t("settings.about.reset.description")}
      >
        <Button variant="outline" size="sm" onClick={resetPrefs}>
          <RotateCcw aria-hidden="true" />
          {t("settings.about.reset.action")}
        </Button>
      </Row>
    </div>
  );
}
