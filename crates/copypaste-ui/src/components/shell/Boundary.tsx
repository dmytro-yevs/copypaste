/**
 * INV-20 — **the shell chrome is never inside an error boundary.**
 *
 * Navigation and the main pane get their own *sibling* boundaries, so a crash
 * in a screen cannot take navigation down with it, and the fallback renders
 * inside the shell layout rather than against a bare document body
 * (CopyPaste-8ebg.12).
 *
 * The boundary itself is `react-error-boundary` rather than a hand-written
 * class component (CLAUDE.md rule 1): it is the maintained package for exactly
 * this, and it brings the reset semantics we would otherwise write badly.
 */
import type { ReactNode } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";

interface BoundaryProps {
  /** Named so a crash report can say *which* region failed. */
  label: string;
  children: ReactNode;
}

export function Boundary({ label, children }: BoundaryProps) {
  return (
    <ErrorBoundary
      // The raw error is logged, never rendered: a stack can contain a bundle
      // path, and a path discloses the local username (INV-12).
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
