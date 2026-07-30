/**
 * The accessible name is the whole sentence, not the state word alone: a screen
 * reader user gets what the tooltip gives a pointer user. No error string is
 * interpolated into it (INV-12).
 */
import { useStatus } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import { classifyError } from "@/lib/errors";
import { cn } from "@/lib/cn";
import { CURRENT_PROTOCOL_VERSION } from "@/lib/ipc";

type ChipState = "running" | "paused" | "starting" | "offline" | "error";

const DOT: Record<ChipState, string> = {
  running: "bg-ok",
  paused: "bg-warn",
  starting: "bg-warn",
  offline: "bg-err",
  error: "bg-err",
};

const STATE_KEY = {
  running: "shell.status.running",
  paused: "shell.status.paused",
  starting: "shell.status.starting",
  offline: "shell.status.offline",
  error: "shell.status.error",
} as const satisfies Record<ChipState, string>;

export function StatusChip() {
  const { t } = useTranslation();
  const status = useStatus();

  let state: ChipState;
  let detail: string;

  if (status.error) {
    const kind = classifyError(status.error);
    state = kind === "offline" ? "offline" : kind === "not_ready" ? "starting" : "error";
    detail =
      state === "offline"
        ? t("shell.status.detail.offline")
        : state === "starting"
          ? t("shell.status.detail.starting")
          : t("shell.status.detail.error");
  } else if (status.data === undefined) {
    state = "starting";
    detail = t("shell.status.detail.connecting");
  } else if (!status.data.capture_running) {
    state = "paused";
    detail = t("shell.status.detail.paused");
  } else {
    state = "running";
    const mismatch = status.data.protocol_version !== CURRENT_PROTOCOL_VERSION;
    detail = mismatch
      ? t("shell.status.detail.mismatch", { version: status.data.version })
      : t("shell.status.detail.running", {
          version: status.data.version,
          count: status.data.item_count,
        });
  }

  const label = t(STATE_KEY[state]);

  return (
    <div
      role="status"
      aria-label={t("shell.status.label", { state: label, detail })}
      title={detail}
      className="flex items-center gap-s-2 rounded-md px-s-2 py-s-1 text-xs text-muted-foreground"
    >
      <span
        aria-hidden="true"
        className={cn("size-[var(--sz-badge-dot)] shrink-0 rounded-full", DOT[state])}
      />
      <span className="truncate">{label}</span>
    </div>
  );
}
