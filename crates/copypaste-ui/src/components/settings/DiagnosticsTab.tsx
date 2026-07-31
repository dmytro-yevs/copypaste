/**
 * What is running, what has been thrown away, and one block to paste into an
 * issue. Not a log viewer — `service::diagnostics` carries that argument.
 *
 * No path (rule 4): nothing here renders a location, and every free-text field
 * is scrubbed in Rust before it arrives, so there is one redactor rather than
 * a second one here.
 *
 * No clipping: every number below is a count, which is what makes a copy
 * button safe to offer at all.
 */
import { ClipboardCopy, RotateCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Row } from "@/components/settings/Row";
import { Section } from "@/components/settings/Section";
import {
  useCopyReport,
  useDiagnostics,
  useSweepNotices,
} from "@/hooks/useDiagnostics";
import { useTranslation } from "@/i18n";
import { isUnavailable } from "@/lib/errors";
import { shortAge } from "@/lib/format";
import type {
  DiagnosticCounters,
  Diagnostics,
  DiagnosticsStatus,
} from "@/service/diagnostics";

/** Backend names that mean the real system clipboard. Anything else is a test
 *  backend and must not be mistaken for one. */
const REAL_BACKENDS = /pasteboard|nspasteboard|system/i;

export function DiagnosticsTab() {
  const { t } = useTranslation();
  const query = useDiagnostics();
  useSweepNotices();

  if (query.error !== null && isUnavailable(query.error)) {
    return (
      <p className="py-s-3 text-sm text-muted-foreground">
        {t("settings.diagnostics.unavailable")}
      </p>
    );
  }

  const data = query.data;
  if (data === undefined) {
    return (
      <p className="py-s-3 text-sm text-muted-foreground">
        {query.error !== null
          ? t("errors.offline")
          : t("settings.diagnostics.loading")}
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-s-4">
      <Section title={t("settings.diagnostics.running.title")}>
        <ServiceRow data={data} />
        <CaptureRows status={data.status} />
        <ProtocolRow data={data} />
        <StartedRow counters={data.status?.counters} />
      </Section>

      <Section
        title={t("settings.diagnostics.dropped.title")}
        description={t("settings.diagnostics.dropped.description")}
      >
        <DroppedRows status={data.status} />
      </Section>

      <ReportSection
        report={data.report}
        onRefresh={() => void query.refetch()}
      />
    </div>
  );
}

function ServiceRow({ data }: { data: Diagnostics }) {
  const { t } = useTranslation();
  const service = data.service;

  const [label, variant] = (() => {
    switch (service.state) {
      case "running":
        return ["running", "ok"] as const;
      case "unhealthy":
        return ["unhealthy", "error"] as const;
      case "stopped":
        return ["stopped", "warn"] as const;
      case "not_installed":
        return ["notInstalled", "warn"] as const;
    }
  })();

  const notes: string[] = [];
  if (service.state === "running") {
    if (!service.matches_app) {
      notes.push(t("settings.diagnostics.running.service.mismatch"));
    }
    if (!service.ours) {
      notes.push(t("settings.diagnostics.running.service.adopted"));
    }
  }

  return (
    <Row
      title={t("settings.diagnostics.running.service.title")}
      note={
        notes.length > 0 ? (
          <span className="text-xs text-warn-strong">{notes.join(" ")}</span>
        ) : undefined
      }
    >
      <span className="flex items-center gap-s-2">
        <Badge variant={variant}>
          {t(`settings.diagnostics.running.service.${label}`)}
        </Badge>
        <span className="text-sm tabular-nums text-muted-foreground">
          {t("settings.diagnostics.running.service.version", {
            version:
              service.state === "running" ? service.version : data.app_version,
          })}
        </span>
      </span>
    </Row>
  );
}

function CaptureRows({ status }: { status: DiagnosticsStatus | null }) {
  const { t } = useTranslation();
  const running = status?.capture_running === true;
  const backendIsReal =
    status === null || REAL_BACKENDS.test(status.clipboard_backend);

  return (
    <>
      <Row
        title={t("settings.diagnostics.running.capture.title")}
        description={t("settings.diagnostics.running.capture.description")}
      >
        {status === null ? (
          <Badge variant="secondary">{t("common.unknown")}</Badge>
        ) : (
          <Badge variant={running ? "ok" : "warn"}>
            {t(
              running
                ? "settings.diagnostics.running.capture.running"
                : "settings.diagnostics.running.capture.stopped",
            )}
          </Badge>
        )}
      </Row>

      <Row
        title={t("settings.diagnostics.running.backend.title")}
        description={t("settings.diagnostics.running.backend.description")}
      >
        {status === null ? (
          <Badge variant="secondary">{t("common.unknown")}</Badge>
        ) : (
          <Badge variant={backendIsReal ? "secondary" : "warn"}>
            {status.clipboard_backend}
          </Badge>
        )}
      </Row>
    </>
  );
}

function ProtocolRow({ data }: { data: Diagnostics }) {
  const { t } = useTranslation();
  const agree =
    data.status !== null &&
    data.status.protocol_version === data.app_protocol_version;

  return (
    <Row
      title={t("settings.diagnostics.running.protocol.title")}
      description={t("settings.diagnostics.running.protocol.description")}
    >
      {data.status === null ? (
        <Badge variant="secondary">{t("common.unknown")}</Badge>
      ) : (
        <Badge variant={agree ? "ok" : "error"}>
          {t(
            agree
              ? "settings.diagnostics.running.protocol.yes"
              : "settings.diagnostics.running.protocol.no",
          )}
        </Badge>
      )}
    </Row>
  );
}

function StartedRow({ counters }: { counters: DiagnosticCounters | undefined }) {
  const { t } = useTranslation();
  return (
    <Row
      title={t("settings.diagnostics.running.started.title")}
      description={t("settings.diagnostics.running.started.description")}
    >
      <span className="text-sm tabular-nums text-muted-foreground">
        {counters === undefined
          ? t("settings.diagnostics.running.started.unknown")
          : shortAge(Date.now() - counters.uptime_secs * 1000)}
      </span>
    </Row>
  );
}

function DroppedRows({ status }: { status: DiagnosticsStatus | null }) {
  const { t } = useTranslation();
  if (status === null) {
    return (
      <p className="py-s-3 text-sm text-muted-foreground">
        {t("errors.offline")}
      </p>
    );
  }

  const c = status.counters;
  return (
    <>
      <CountRow name="tooLarge" count={c.rejected_too_large} />
      <CountRow name="missed" count={c.lost_intermediates} />
      <CountRow name="swept" count={c.sensitive_swept} />
      <CountRow name="purged" count={c.index_purged} />
      {status.legacy_history_present && (
        <Row
          title={t("settings.diagnostics.dropped.legacy.title")}
          description={t("settings.diagnostics.dropped.legacy.description")}
        >
          <Badge variant="info">
            {t("settings.diagnostics.dropped.legacy.present")}
          </Badge>
        </Row>
      )}
    </>
  );
}

/** Zero is rendered, never hidden: "nothing was dropped" is the answer the
 *  panel most often exists to give, and a row that only appears when it is
 *  non-zero is one nobody knows to look for. */
function CountRow({
  name,
  count,
}: {
  name: "tooLarge" | "missed" | "swept" | "purged";
  count: number;
}) {
  const { t } = useTranslation();
  return (
    <Row
      title={t(`settings.diagnostics.dropped.${name}.title`)}
      description={t(`settings.diagnostics.dropped.${name}.description`)}
    >
      <span
        className={
          count > 0
            ? "text-sm font-medium tabular-nums text-warn-strong"
            : "text-sm tabular-nums text-muted-foreground"
        }
      >
        {count.toLocaleString()}
      </span>
    </Row>
  );
}

/**
 * The report is shown before it is copied. A user about to paste something into
 * a public issue is entitled to read it first, and showing it is also the only
 * honest way to make the "no clipping, nothing that names you" claim beside the
 * button.
 */
function ReportSection({
  report,
  onRefresh,
}: {
  report: string;
  onRefresh: () => void;
}) {
  const { t } = useTranslation();
  const copy = useCopyReport();
  const empty = report.trim() === "";

  return (
    <Section
      title={t("settings.diagnostics.report.title")}
      description={t("settings.diagnostics.report.description")}
    >
      <pre className="my-s-2 max-h-64 overflow-auto whitespace-pre rounded-md border border-divider bg-panel p-s-2 text-xs leading-relaxed">
        {empty ? t("settings.diagnostics.report.empty") : report}
      </pre>
      <p className="pb-s-2 text-xs text-muted-foreground">
        {t("settings.diagnostics.report.safety")}
      </p>
      <div className="flex flex-wrap items-center gap-s-2">
        <Button
          size="sm"
          disabled={copy.isPending || empty}
          onClick={() => copy.mutate(report)}
        >
          <ClipboardCopy aria-hidden="true" />
          {t("settings.diagnostics.report.copy")}
        </Button>
        <Button variant="outline" size="sm" onClick={onRefresh}>
          <RotateCw aria-hidden="true" />
          {t("settings.diagnostics.refresh")}
        </Button>
      </div>
    </Section>
  );
}
