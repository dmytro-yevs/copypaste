import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { lazy, Suspense, type ReactNode } from "react";

import { AppToaster } from "@/app/shell/AppToaster";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ViewportMetricsProvider } from "@/hooks/useViewportMetrics";
import { IpcFailure } from "@/lib/errors";
import { currentPlatform } from "@/lib/platform";

const RETRY_BACKOFF_MS = [250, 500, 1000, 2000, 2000];
const PreviewDevtools = import.meta.env.DEV
  ? lazy(() => import("@/devtools/PreviewDevtools"))
  : null;

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchIntervalInBackground: false,
      refetchOnWindowFocus: true,
      structuralSharing: true,
      retry: (failureCount, error) => error instanceof IpcFailure && error.kind === "not_ready" && failureCount < RETRY_BACKOFF_MS.length,
      retryDelay: (attempt) => RETRY_BACKOFF_MS[attempt] ?? 2000,
    },
    mutations: { retry: false },
  },
});

function applyRootCapabilities(): void {
  const root = document.documentElement;
  root.dataset.platform = currentPlatform();
}

applyRootCapabilities();

export function AppProviders({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>
      <TooltipProvider delayDuration={500}>
        <ViewportMetricsProvider>
          {children}
          <AppToaster />
          {PreviewDevtools ? (
            <Suspense fallback={null}>
              <PreviewDevtools />
            </Suspense>
          ) : null}
        </ViewportMetricsProvider>
      </TooltipProvider>
    </QueryClientProvider>
  );
}
