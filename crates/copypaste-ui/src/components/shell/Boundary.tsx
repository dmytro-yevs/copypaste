/** INV-20: sibling boundaries around navigation and the main pane, so a crash
 *  in a screen cannot take navigation down with it (CopyPaste-8ebg.12). */
import type { ReactNode } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";

interface BoundaryProps {
  /** Named so a crash report says which region failed. */
  label: string;
  children: ReactNode;
}

export function Boundary({ label, children }: BoundaryProps) {
  return (
    <ErrorBoundary
      // Logged, never rendered: a stack contains a bundle path (INV-12).
      onError={(error) => console.error(`[copypaste] ${label} crashed`, error)}
      fallbackRender={({ resetErrorBoundary }) => (
        <div
          role="alert"
          className="flex min-h-0 flex-1 flex-col items-center justify-center gap-s-3 p-[var(--pad-empty)] text-center"
        >
          <p className="text-lg font-medium">Something went wrong here</p>
          <p className="max-w-[var(--content-max-width)] text-sm text-muted-foreground">
            {label} stopped working. The rest of CopyPaste is still running, and
            your clipboard history is untouched.
          </p>
          <Button variant="outline" onClick={resetErrorBoundary}>
            <RotateCcw aria-hidden="true" />
            Reload this view
          </Button>
        </div>
      )}
    >
      {children}
    </ErrorBoundary>
  );
}
