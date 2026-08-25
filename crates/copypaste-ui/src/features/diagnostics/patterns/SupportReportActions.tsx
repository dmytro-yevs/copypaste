import { Icon } from "@/components/ui/icon";
import { Button } from "@/components/ui";
import {
  useCopyReport,
  useDiagnostics,
  useExportSupportBundle,
} from "@/hooks/useDiagnostics";
import { useTranslation } from "@/i18n";
import styles from "./SupportReportActions.module.css";

type SupportReportActionsProps = {
  report: string | undefined;
  compact?: boolean;
};

/** Copy writes the displayed diagnostics; export creates a native support
 * bundle with separate, bounded and redacted runtime-event sections. */
export function SupportReportActions({
  report,
  compact = false,
}: SupportReportActionsProps) {
  const { t } = useTranslation();
  const copy = useCopyReport();
  const exportReport = useExportSupportBundle();
  const empty = report === undefined || report.trim() === "";

  return (
    <div className={styles.actions}>
      <Button
        size={compact ? "sm" : "md"}
        variant="secondary"
        disabled={copy.isPending || empty}
        onClick={() => {
          if (report !== undefined) copy.mutate(report);
        }}
      >
        <Icon name="copy" aria-hidden="true" />
        {t("settings.diagnostics.report.copy")}
      </Button>
      <Button
        size={compact ? "sm" : "md"}
        variant="secondary"
        disabled={exportReport.isPending}
        onClick={() => exportReport.mutate()}
      >
        <Icon name="download" aria-hidden="true" />
        {t(
          exportReport.isPending
            ? "settings.diagnostics.report.exporting"
            : "settings.diagnostics.report.export",
        )}
      </Button>
    </div>
  );
}

/** Loaded only on a recovery screen, so normal history browsing does not add a
 * second diagnostics poll solely for an action that is not visible. */
export function RecoveryReportActions() {
  const diagnostics = useDiagnostics();
  return <SupportReportActions report={diagnostics.data?.report} compact />;
}
