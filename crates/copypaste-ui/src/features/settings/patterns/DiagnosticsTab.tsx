/** Diagnostic reports contain counts only; Rust redacts free text before it arrives. */
import { useState } from "react";
import { Badge, Button, Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui";
import { StateView } from "@/components/shared/StateView";
import { SupportReportActions } from "@/features/diagnostics";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { RuntimeEventsTab } from "@/features/settings/patterns/RuntimeEventsTab";
import { useDiagnostics, useSweepNotices } from "@/hooks/useDiagnostics";
import { useTranslation } from "@/i18n";
import { isUnavailable } from "@/lib/errors";
import { shortAge } from "@/lib/format";
import type { Diagnostics } from "@/service/diagnostics";
import styles from "./DiagnosticsTab.module.css";

export type DiagnosticsView = "overview" | "runtime-events";

export function DiagnosticsTab({ view = "overview", onOpenEvents, onBack }: { view?: DiagnosticsView; onOpenEvents?: () => void; onBack?: () => void }) {
  const { t } = useTranslation();
  if (view === "runtime-events") return <div className={`${styles.overview} ${styles.events}`} data-settings-search-target={`row:${t("runtimeLog.title")}`}>
    {onBack ? <Button type="button" variant="ghost" size="sm" onClick={onBack}>{t("runtimeLog.back")}</Button> : null}
    <h2 data-settings-search-target={`section:${t("runtimeLog.title")}`} className={styles.eventsTitle}>{t("runtimeLog.title")}</h2>
    <RuntimeEventsTab />
  </div>;
  return <div className={styles.overview}><DiagnosticsOverview />{onOpenEvents ? <SettingsSchemaRenderer groups={[{
    id: "runtime-events", title: t("runtimeLog.title"), description: t("runtimeLog.description"), fields: [{
      kind: "action", definition: settingDefinition("runtime-events", "runtimeLog.title"), label: t("runtimeLog.open"), onAction: onOpenEvents,
    }],
  }]} /> : null}</div>;
}

function DiagnosticsOverview() {
  const { t } = useTranslation();
  const query = useDiagnostics();
  useSweepNotices();
  const data = query.data;
  if (data === undefined) {
    if (query.error === null) return <StateView mode="loading" placement="panel" title={t("settings.diagnostics.loading")} />;
    return <StateView mode={isUnavailable(query.error) ? "offline" : "error"} placement="panel" role="alert" title={t("settings.diagnostics.unavailable")} description={t(isUnavailable(query.error) ? "errors.offline" : "settings.diagnostics.errorBody")} actions={<Button variant="secondary" onClick={() => void query.refetch()}>{t("common.tryAgain")}</Button>} />;
  }
  const counters = data.status?.counters;
  return <div className={styles.overview}>
    {query.isFetching ? <StateView mode="loading" placement="inline" title="Refreshing diagnostics…" /> : null}
    <SettingsSchemaRenderer groups={[
      { id: "running", title: t("settings.diagnostics.running.title"), fields: [
        { kind: "status", definition: settingDefinition("diagnostics", "settings.diagnostics.running.history.title"), value: <Badge variant={data.history_read.state === "readable" ? "ok" : "error"}>{data.history_read.state === "readable" ? t("settings.diagnostics.running.history.readable") : t("settings.diagnostics.running.history.failed", { code: data.history_read.code })}</Badge> },
        { kind: "readonly", definition: settingDefinition("diagnostics", "settings.diagnostics.running.started.title"), value: <span className={styles.metric}>{counters === undefined ? t("settings.diagnostics.running.started.unknown") : shortAge(Date.now() - counters.uptime_secs * 1000)}</span> },
      ] },
      { id: "dropped", title: t("settings.diagnostics.dropped.title"), description: t("settings.diagnostics.dropped.description"), fields: data.status === null ? [{ kind: "status", definition: settingDefinition("diagnostics", "settings.diagnostics.dropped.tooLarge.title"), value: <span className={styles.offline}>{t("errors.offline")}</span> }] : ([
        ["tooLarge", counters?.rejected_too_large], ["missed", counters?.lost_intermediates], ["swept", counters?.sensitive_swept], ["purged", counters?.index_purged],
      ] as const).map(([name, count]) => ({ kind: "readonly" as const, definition: settingDefinition("diagnostics", `settings.diagnostics.dropped.${name}.title`), value: <span className={(count ?? 0) > 0 ? styles.warningCount : styles.count}>{(count ?? 0).toLocaleString()}</span> })) },
    ]} />
    <ReportSection report={data.report} />
  </div>;
}

function ReportSection({ report }: { report: Diagnostics["report"] }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const empty = report.trim() === "";
  return <>
    <SettingsSchemaRenderer groups={[{ id: "support", title: "Support", fields: [{
      kind: "action", definition: settingDefinition("diagnostics", "settings.diagnostics.report.title"), label: "Open", onAction: () => setOpen(true),
    }] }]} />
    <Dialog open={open} onOpenChange={setOpen}><DialogContent><DialogHeader><DialogTitle>{t("settings.diagnostics.report.title")}</DialogTitle><DialogDescription>{t("settings.diagnostics.report.description")}</DialogDescription></DialogHeader><pre className={styles.report}>{empty ? t("settings.diagnostics.report.empty") : report}</pre><p className={styles.safety}>{t("settings.diagnostics.report.safety")}</p><SupportReportActions report={empty ? undefined : report} compact /></DialogContent></Dialog>
  </>;
}
