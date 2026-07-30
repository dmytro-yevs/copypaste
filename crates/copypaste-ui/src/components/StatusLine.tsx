/**
 * The bottom status line.
 *
 * Vocabulary is the canonical one from manifest §3.2.5: "clipboard service" /
 * "background service", never "daemon". The text is never derived from an
 * error object (INV-12) — the connection state is a token, and the token
 * chooses the sentence.
 *
 * `clipboard_backend` is surfaced because `copypaste-ipc` says it exists so a
 * demo cannot be mistaken for the real thing: a fake backend is called out in
 * the warning colour rather than left to look like a working clipboard.
 */
import type { ErrorKind } from "../lib/errors";
import type { StatusData } from "../lib/ipc";

interface StatusLineProps {
  status: StatusData | undefined;
  error: ErrorKind | null;
  /** Rows currently on screen; shown when a search narrows the daemon total. */
  visible: number;
  filtered: boolean;
}

const REAL_BACKENDS = /pasteboard|nspasteboard|system/i;

export function StatusLine({
  status,
  error,
  visible,
  filtered,
}: StatusLineProps) {
  const connected = error === null && status !== undefined;
  const backendIsReal = status ? REAL_BACKENDS.test(status.clipboard_backend) : true;

  let connection: string;
  let tone: string;
  if (error === "offline") {
    connection = "Background service not running";
    tone = "bg-err";
  } else if (error === "not_ready") {
    connection = "Clipboard service starting up";
    tone = "bg-warn";
  } else if (error !== null) {
    connection = "Clipboard service error";
    tone = "bg-err";
  } else if (status === undefined) {
    connection = "Connecting…";
    tone = "bg-mute";
  } else if (!status.capture_running) {
    connection = "Clipboard capture paused";
    tone = "bg-warn";
  } else {
    connection = "Clipboard service running";
    tone = "bg-ok";
  }

  return (
    <footer
      className="flex shrink-0 items-center gap-s-3 border-t border-divider bg-panel px-s-4 py-s-2 text-fs-xs text-faint"
      aria-label="Clipboard service status"
    >
      <span
        aria-hidden="true"
        className={`size-[var(--sz-badge-dot)] shrink-0 rounded-pill ${tone}`}
      />
      <span className="truncate">{connection}</span>

      {connected && !backendIsReal && (
        <span
          className="shrink-0 rounded-chip bg-elevated px-[7px] py-[2px] text-warn-strong"
          title="This build is not reading the real system clipboard."
        >
          Test clipboard backend
        </span>
      )}

      <span className="ml-auto shrink-0 tabular-nums">
        {filtered
          ? `${visible} shown`
          : status
            ? `${status.item_count} ${status.item_count === 1 ? "item" : "items"}`
            : `${visible} ${visible === 1 ? "item" : "items"}`}
      </span>

      {status && (
        <span className="shrink-0">CopyPaste {status.version}</span>
      )}
    </footer>
  );
}
