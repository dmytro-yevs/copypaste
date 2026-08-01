import { ClipboardCopy, Download } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  useCopyReport,
  useDiagnostics,
  useExportSupportBundle,
} from "@/hooks/useDiagnostics";
import { useTranslation } from "@/i18n";

interface SupportReportActionsProps {
  report: string | undefined;
  compact?: boolean;
}

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
    <div className="flex flex-wrap justify-center gap-s-2">
      <Button
        size={compact ? "sm" : "default"}
        variant="outline"
        disabled={copy.isPending || empty}
        onClick={() => {
          if (report !== undefined) copy.mutate(report);
        }}
      >
        <ClipboardCopy aria-hidden="true" />
        {t("settings.diagnostics.report.copy")}
      </Button>
      <Button
        size={compact ? "sm" : "default"}
        variant="outline"
        disabled={exportReport.isPending}
        onClick={() => exportReport.mutate()}
      >
        <Download aria-hidden="true" />
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
