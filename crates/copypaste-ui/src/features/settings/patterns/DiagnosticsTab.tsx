/** Diagnostic reports contain counts only; Rust redacts free text before it arrives. */
import { useState } from "react";
import { Button, Dialog } from "@/components/ui";
import { StateView } from "@/components/shared/StateView";
import { SupportReportActions } from "@/features/diagnostics";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { settingsGroups } from "@/features/settings/model/settingsProjection";
import type { SettingsField } from "@/features/settings/model/settingsFieldSchema";
import { RuntimeEventsTab } from "@/features/settings/patterns/RuntimeEventsTab";
import { useDiagnostics } from "@/hooks/useDiagnostics";
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
  return <div className={styles.overview}><DiagnosticsOverview />{onOpenEvents ? <SettingsSchemaRenderer groups={settingsGroups("diagnostics", [{
    kind: "action", definition: settingDefinition("runtime-events", "runtimeLog.title"), label: t("runtimeLog.open"), onAction: onOpenEvents,
  }], (key) => t(key as never))} /> : null}</div>;
}

function DiagnosticsOverview() {
  const { t } = useTranslation();
  const query = useDiagnostics();
  const data = query.data;
  if (data === undefined) {
    if (query.error === null) return <StateView mode="loading" placement="panel" title={t("settings.diagnostics.loading")} />;
    return <StateView mode={isUnavailable(query.error) ? "offline" : "error"} placement="panel" role="alert" title={t("settings.diagnostics.unavailable")} description={t(isUnavailable(query.error) ? "errors.offline" : "settings.diagnostics.errorBody")} actions={<Button variant="secondary" onClick={() => void query.refetch()}>{t("common.tryAgain")}</Button>} />;
  }
  const counters = data.status?.counters;
  const fields: SettingsField[] = [
    { kind: "status", definition: settingDefinition("diagnostics", "settings.diagnostics.running.history.title"), mode: data.history_read.state === "readable" ? "success" : "error", value: data.history_read.state === "readable" ? t("settings.diagnostics.running.history.readable") : t("settings.diagnostics.running.history.failed", { code: data.history_read.code }) },
    { kind: "readonly", definition: settingDefinition("diagnostics", "settings.diagnostics.running.started.title"), value: <span className={styles.metric}>{counters === undefined ? t("settings.diagnostics.running.started.unknown") : shortAge(Date.now() - counters.uptime_secs * 1000)}</span> },
    ...(data.status === null ? [{ kind: "status" as const, definition: settingDefinition("diagnostics", "settings.diagnostics.dropped.tooLarge.title"), mode: "offline" as const, value: t("errors.offline") }] : ([
      ["tooLarge", counters?.rejected_too_large], ["missed", counters?.lost_intermediates],
    ] as const).map(([name, count]) => ({ kind: "readonly" as const, definition: settingDefinition("diagnostics", `settings.diagnostics.dropped.${name}.title`), value: <span className={(count ?? 0) > 0 ? styles.warningCount : styles.count}>{(count ?? 0).toLocaleString()}</span> }))),
  ];
  return <div className={styles.overview}>
    {query.isFetching ? <StateView mode="loading" placement="inline" title="Refreshing diagnostics…" /> : null}
    <SettingsSchemaRenderer groups={settingsGroups("diagnostics", fields, (key) => t(key as never))} />
    <ReportSection report={data.report} />
  </div>;
}

function ReportSection({ report }: { report: Diagnostics["report"] }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const empty = report.trim() === "";
  return <>
    <SettingsSchemaRenderer groups={settingsGroups("diagnostics", [{
      kind: "action", definition: settingDefinition("diagnostics", "settings.diagnostics.report.title"), label: "Open", onAction: () => setOpen(true),
    }], (key) => t(key as never))} />
    <Dialog open={open} onOpenChange={setOpen} title={t("settings.diagnostics.report.title")} description={t("settings.diagnostics.report.description")}>{empty ? <StateView mode="empty" placement="inline" title={t("settings.diagnostics.report.empty")} /> : <pre className={styles.report}>{report}</pre>}<p className={styles.safety}>{t("settings.diagnostics.report.safety")}</p><SupportReportActions report={empty ? undefined : report} compact /></Dialog>
  </>;
}
