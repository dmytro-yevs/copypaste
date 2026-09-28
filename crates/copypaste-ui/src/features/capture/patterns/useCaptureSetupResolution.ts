import { useCaptureState } from "@/hooks/useCapture";
import type { CaptureSnapshot } from "@/lib/ipc";

type CaptureSetupResolution =
  | { kind: "ready"; snapshot: CaptureSnapshot }
  | { kind: "loading" }
  | { kind: "error"; retry: () => void };

/** Keeps query precedence and retry outside the shared state presentation. */
export function useCaptureSetupResolution(): CaptureSetupResolution {
  const capture = useCaptureState();
  if (capture.data !== undefined) return { kind: "ready", snapshot: capture.data };
  if (capture.isPending) return { kind: "loading" };
  return { kind: "error", retry: () => void capture.refetch() };
}
