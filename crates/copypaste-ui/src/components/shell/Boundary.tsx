import type { ReactNode } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { RotateCcw, TriangleAlert } from "lucide-react";

import { EmptyState } from "@/components/EmptyState";
import { RecoveryReportActions } from "@/components/diagnostics/SupportReportActions";
import { useTranslation } from "@/i18n";

interface BoundaryProps {
  /** Named so a crash report says which region failed. */
  label: string;
  children: ReactNode;
}

export function Boundary({ label, children }: BoundaryProps) {
  const { t } = useTranslation();

  return (
    <ErrorBoundary
      // Logged, never rendered: a stack contains a bundle path (INV-12).
      onError={(error) => console.error(`[copypaste] ${label} crashed`, error)}
      fallbackRender={({ resetErrorBoundary }) => (
        <EmptyState
          icon={TriangleAlert}
          tone="danger"
          title={t("shell.boundary.title")}
          body={t("shell.boundary.body", { region: label })}
          action={{
            label: t("shell.boundary.reload"),
            icon: RotateCcw,
            onClick: resetErrorBoundary,
          }}
          secondary={<RecoveryReportActions />}
        />
      )}
    >
      {children}
    </ErrorBoundary>
  );
}
