import type { ReactNode } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";
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
        <div
          role="alert"
          className="flex min-h-0 flex-1 flex-col items-center justify-center gap-s-3 p-[var(--pad-empty)] text-center"
        >
          <p className="text-lg font-medium">{t("shell.boundary.title")}</p>
          <p className="max-w-[var(--content-max-width)] text-sm text-muted-foreground">
            {t("shell.boundary.body", { region: label })}
          </p>
          <Button variant="outline" onClick={resetErrorBoundary}>
            <RotateCcw aria-hidden="true" />
            {t("shell.boundary.reload")}
          </Button>
        </div>
      )}
    >
      {children}
    </ErrorBoundary>
  );
}
