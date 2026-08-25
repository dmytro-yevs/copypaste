/**
 * No path (rule 4): nothing here renders a location, and every free-text field
 * is scrubbed in Rust before it arrives, so there is one redactor rather than a
 * second one here.
 *
 * No clipping: every number below is a count, which is what makes a copy button
 * safe to offer at all.
 */

import { useState } from "react";

import { IllustratedErrorState, SettingsRow } from "@/components/shared";
import {
  Badge,
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui";
import { SupportReportActions } from "@/features/diagnostics";
import { Section } from "@/features/settings/components/Section";
import {
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
import styles from "./DiagnosticsTab.module.css";

export function DiagnosticsTab() {
  const { t } = useTranslation();
  const query = useDiagnostics();
  useSweepNotices();

  const data = query.data;
  if (data === undefined) {
    if (query.error === null) return <DiagnosticsLoadingSkeleton />;
    return (
      <IllustratedErrorState
        compact
        title={t("settings.diagnostics.unavailable")}
        body={t(
          isUnavailable(query.error)
            ? "errors.offline"
            : "settings.diagnostics.errorBody",
        )}
        actions={
          <Button variant="secondary" onClick={() => void query.refetch()}>
            {t("common.tryAgain")}
          </Button>
        }
      />
    );
  }

  return (
    <div className={styles.overview}>
      {query.isFetching ? (
        <p role="status" className={styles.refreshing}>Refreshing diagnostics…</p>
      ) : null}
      <Section title={t("settings.diagnostics.running.title")}>
        <HistoryReadRow historyRead={data.history_read} />
        <StartedRow counters={data.status?.counters} />
      </Section>

      <Section
        title={t("settings.diagnostics.dropped.title")}
        description={t("settings.diagnostics.dropped.description")}
      >
        <DroppedRows status={data.status} />
      </Section>

      <ReportSection report={data.report} />
    </div>
  );
}

function DiagnosticsLoadingSkeleton() {
  const { t } = useTranslation();
  return (
    <div
      className={styles.loading}
      role="status"
      aria-label={t("settings.diagnostics.loading")}
      aria-busy="true"
    >
      {[2, 4, 1].map((rowCount, sectionIndex) => (
        <section className={styles.loadingSection} key={`${rowCount}-${sectionIndex}`}>
          <span className={styles.loadingHeading} />
          <div className={styles.loadingSurface}>
            {Array.from({ length: rowCount }, (_, rowIndex) => (
              <div className={styles.loadingRow} key={rowIndex}>
                <span className={styles.loadingCopy}><i /><i /></span>
                <span className={styles.loadingValue} />
              </div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

function HistoryReadRow({
  historyRead,
}: {
  historyRead: Diagnostics["history_read"];
}) {
  const { t } = useTranslation();
  const readable = historyRead.state === "readable";

  return (
    <SettingsRow
      title={t("settings.diagnostics.running.history.title")}
      description={t("settings.diagnostics.running.history.description")}
    >
      <Badge variant={readable ? "ok" : "error"}>
        {readable
          ? t("settings.diagnostics.running.history.readable")
          : t("settings.diagnostics.running.history.failed", {
              code: historyRead.code,
            })}
      </Badge>
    </SettingsRow>
  );
}

function StartedRow({ counters }: { counters: DiagnosticCounters | undefined }) {
  const { t } = useTranslation();
  return (
    <SettingsRow
      title={t("settings.diagnostics.running.started.title")}
      description={t("settings.diagnostics.running.started.description")}
    >
      <span className={styles.metric}>
        {counters === undefined
          ? t("settings.diagnostics.running.started.unknown")
          : shortAge(Date.now() - counters.uptime_secs * 1000)}
      </span>
    </SettingsRow>
  );
}

function DroppedRows({ status }: { status: DiagnosticsStatus | null }) {
  const { t } = useTranslation();
  if (status === null) {
    return (
      <p className={styles.offline}>
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
    <SettingsRow
      title={t(`settings.diagnostics.dropped.${name}.title`)}
      description={t(`settings.diagnostics.dropped.${name}.description`)}
    >
      <span
        className={count > 0 ? styles.warningCount : styles.count}
      >
        {count.toLocaleString()}
      </span>
    </SettingsRow>
  );
}

/** The report is shown before it is copied: a user about to paste something
 *  into a public issue is entitled to read it first, and showing it is the only
 *  honest way to make the claim printed beside the button. */
function ReportSection({ report }: { report: string }) {
  const { t } = useTranslation();
  const empty = report.trim() === "";
  const [open, setOpen] = useState(false);

  return (
    <Section
      title="Support"
    >
      <SettingsRow
        title={t("settings.diagnostics.report.title")}
        description={t("settings.diagnostics.report.description")}
      >
        <Button type="button" variant="secondary" size="sm" onClick={() => setOpen(true)}>
          Open
        </Button>
      </SettingsRow>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("settings.diagnostics.report.title")}</DialogTitle>
            <DialogDescription>
              {t("settings.diagnostics.report.description")}
            </DialogDescription>
          </DialogHeader>
          <pre className={styles.report}>
            {empty ? t("settings.diagnostics.report.empty") : report}
          </pre>
          <p className={styles.safety}>
            {t("settings.diagnostics.report.safety")}
          </p>
          <SupportReportActions report={empty ? undefined : report} compact />
        </DialogContent>
      </Dialog>
    </Section>
  );
}
