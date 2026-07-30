/**
 * `clipboard_backend` is surfaced because a fake backend must not be mistaken
 * for the real pasteboard: a build reading `fake` or `android-inprocess` says
 * so, in the warning colour, rather than looking like a working clipboard.
 *
 * Nothing here renders a path (INV-12), which is why the service is described
 * by version and backend rather than by where it lives.
 */
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useStatus } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import { classifyError, friendlyError } from "@/lib/errors";
import { CURRENT_PROTOCOL_VERSION } from "@/lib/ipc";
import { usePrefs } from "@/store/prefs";
import { Row } from "@/components/settings/Row";

const REAL_BACKENDS = /pasteboard|nspasteboard|system/i;

export function AboutTab() {
  const { t } = useTranslation();
  const status = useStatus();
  const resetPrefs = usePrefs((s) => s.reset);

  const backendIsReal = status.data
    ? REAL_BACKENDS.test(status.data.clipboard_backend)
    : true;
  const mismatch =
    status.data !== undefined &&
    status.data.protocol_version !== CURRENT_PROTOCOL_VERSION;

  return (
    <div className="flex flex-col">
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

      <Row
        title={t("settings.about.reset.title")}
        description={t("settings.about.reset.description")}
      >
        <Button variant="outline" size="sm" onClick={resetPrefs}>
          {t("settings.about.reset.action")}
        </Button>
      </Row>
    </div>
  );
}
