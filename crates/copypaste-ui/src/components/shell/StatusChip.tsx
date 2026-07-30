/**
 * The service chip in the sidebar footer (manifest §3.8).
 *
 * State comes from the status query, which polls at 2000ms so "offline"
 * surfaces within one cycle — a 10s poll left v1's chip green long after the
 * service had died (CopyPaste-f701).
 *
 * The accessible name is the whole sentence, not just the state word, so a
 * screen reader user gets the same information the tooltip gives a pointer
 * user. No error string is ever interpolated into it (INV-12).
 */
import { useStatus } from "@/hooks/useHistory";
import { classifyError } from "@/lib/errors";
import { cn } from "@/lib/cn";
import { CURRENT_PROTOCOL_VERSION } from "@/lib/ipc";

type ChipState = "running" | "paused" | "starting" | "offline" | "error";

const COPY: Record<ChipState, { label: string; dot: string }> = {
  running: { label: "Service running", dot: "bg-ok" },
  paused: { label: "Capture paused", dot: "bg-warn" },
  starting: { label: "Starting up", dot: "bg-warn" },
  offline: { label: "Service offline", dot: "bg-err" },
  error: { label: "Service error", dot: "bg-err" },
};

export function StatusChip() {
  const status = useStatus();

  let state: ChipState;
  let detail: string;

  if (status.error) {
    const kind = classifyError(status.error);
    state = kind === "offline" ? "offline" : kind === "not_ready" ? "starting" : "error";
    detail =
      state === "offline"
        ? "The background service is not running."
        : state === "starting"
          ? "The clipboard service is initializing."
          : "The background service returned an error.";
  } else if (status.data === undefined) {
    state = "starting";
    detail = "Connecting to the background service.";
  } else if (!status.data.capture_running) {
    state = "paused";
    detail = "The service is running but is not recording the clipboard.";
  } else {
    state = "running";
    const mismatch = status.data.protocol_version !== CURRENT_PROTOCOL_VERSION;
    detail = mismatch
      ? `CopyPaste ${status.data.version}, on a different protocol version.`
      : `CopyPaste ${status.data.version} · ${status.data.item_count} item${status.data.item_count === 1 ? "" : "s"}`;
  }

  const { label, dot } = COPY[state];

  return (
    <div
      role="status"
      aria-label={`Clipboard service: ${label}. ${detail}`}
      title={detail}
      className="flex items-center gap-s-2 rounded-md px-s-2 py-s-1 text-xs text-muted-foreground"
    >
      <span
        aria-hidden="true"
        className={cn("size-[var(--sz-badge-dot)] shrink-0 rounded-full", dot)}
      />
      <span className="truncate">{label}</span>
    </div>
  );
}
