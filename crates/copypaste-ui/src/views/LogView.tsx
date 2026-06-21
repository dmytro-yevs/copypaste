import { useState, useEffect, useCallback, useRef } from "react";
import { readLogs, logDirPath, IpcError } from "../lib/ipc";
import { ViewShell } from "../components/ViewShell";
import { RestartDaemonButton } from "../components/RestartDaemonButton";

const MAX_LINES = 500;

// audit P2: colorize each log line by level. Each token meets WCAG AA on the
// log surface (bg-ide-raised): WARN→warning amber, ERROR→danger red, DEBUG/TRACE
// dimmed (faint), INFO + everything else→neutral text. Detection is a simple
// word-boundary scan for the level token tracing emits (case-insensitive).
type LogLevel = "error" | "warn" | "info" | "debug";

function levelOf(line: string): LogLevel {
  // Match a standalone level word (avoids matching "information" etc.).
  if (/\bERROR\b/i.test(line)) return "error";
  if (/\bWARN(?:ING)?\b/i.test(line)) return "warn";
  if (/\b(?:DEBUG|TRACE)\b/i.test(line)) return "debug";
  return "info";
}

const LEVEL_CLASS: Record<LogLevel, string> = {
  error: "text-ide-danger",
  warn: "text-ide-warning",
  info: "text-ide-text",
  debug: "text-ide-faint",
};

// Per-level left-border accent so the row type reads at a glance.
// Uses color-mix via inline style to stay token-only (no hardcoded hex).
const LEVEL_BORDER: Record<LogLevel, string> = {
  error: "border-l-ide-danger",
  warn: "border-l-ide-warning",
  info: "border-l-transparent",
  debug: "border-l-transparent",
};

/**
 * Redact the absolute log directory path before display (CopyPaste-2b3i).
 *
 * Replaces the leading /Users/<username> prefix with ~ so screen recordings,
 * screenshots, and accessibility APIs never expose the local username.
 * The full absolute path is still available as the `title` attribute (tooltip).
 *
 * Works for any /Users/<name>/... path; non-/Users/ paths are returned as-is
 * (they don't contain a macOS username).
 */
function relativizeLogPath(absPath: string): string {
  // Match /Users/<any-username>/ and replace the prefix with ~/
  return absPath.replace(/^\/Users\/[^/]+\//, "~/");
}

export function LogView() {
  const [content, setContent] = useState<string>("");
  // bdac.63: track empty state as a boolean, not a string sentinel.
  // Previously setContent("(no log entries)") tied display text to logic.
  const [isEmpty, setIsEmpty] = useState(false);
  const [logPath, setLogPath] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    try {
      const [logs, path] = await Promise.all([
        readLogs(MAX_LINES),
        logDirPath(),
      ]);
      const empty = !logs || logs.trim().length === 0;
      setIsEmpty(empty);
      setContent(empty ? "" : logs);
      setLogPath(path);
      setError(null);
    } catch (err) {
      // Log raw error for diagnostics — never render raw FS paths or IPC strings
      // in the DOM (CopyPaste-mtvx).
      // eslint-disable-next-line no-console
      console.error("[LogView] failed to load logs:", err);
      if (err instanceof IpcError && err.code === "daemon_offline") {
        setError("The clipboard daemon is not running. Start it to view logs.");
      } else {
        setError("Could not load logs. The daemon may be offline or the log file is unavailable.");
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // Auto-scroll to bottom when content loads
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [content]);

  const handleExport = useCallback(() => {
    const blob = new Blob([content], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "copypaste-daemon.log";
    a.click();
    // Defer revoke so the browser has time to start the download before the
    // object URL is invalidated (immediate revoke can abort the download).
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }, [content]);

  const lines = content.split("\n");
  const lastLineIdx = lines.length - 1;

  // Actions slot rendered into the ViewShell header (Refresh + Export buttons).
  const headerActions = (
    <div className="flex items-center gap-2">
      {/* rounded-ide removed; --skin-r-ctl drives radius so Quiet/Vapor adapt. */}
      <button
        onClick={() => {
          setLoading(true);
          void load();
        }}
        className="border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-dim shadow-ide-xs hover:bg-ide-raised hover:text-ide-text"
        style={{ borderRadius: "var(--skin-r-ctl)" }}
      >
        Refresh
      </button>
      <button
        onClick={handleExport}
        disabled={isEmpty || !content}
        className="border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-dim shadow-ide-xs hover:bg-ide-raised hover:text-ide-text disabled:opacity-40"
        style={{ borderRadius: "var(--skin-r-ctl)" }}
      >
        Export
      </button>
    </div>
  );

  return (
    // SCRL-11: use shared ViewShell for consistent header + drag-region + glass
    // surface. The redundant inner surface-card header is removed (fixes SCRL-3).
    <ViewShell title="Daemon Logs" actions={headerActions}>
      <div className="flex h-full flex-col">
        {/* Path subtitle shown below the ViewShell title, inside the content panel. */}
        {logPath && (
          <p className="mb-2 text-[11px] text-ide-faint" title={relativizeLogPath(logPath)}>
            {relativizeLogPath(logPath)}
          </p>
        )}

        {/* Content */}
        <div className="min-h-0 flex-1">
          {loading ? (
            /* bdac.94: animated spinner replaces static "Loading…" text — matches
               the DevicesView pattern (animate-spin + motion-reduce guard). */
            <div className="flex h-full items-center justify-center">
              <span
                aria-label="Loading logs…"
                className="inline-block h-5 w-5 animate-spin motion-reduce:animate-none rounded-full border-2 border-ide-faint border-t-ide-accent"
              />
            </div>
          ) : error ? (
            /* bdac.96: RestartDaemonButton added to error state so users can recover
               without navigating away — consistent with HistoryView / DevicesView. */
            <div className="flex h-full flex-col items-center justify-center gap-3">
              <p className="text-[13px] text-ide-danger">{error}</p>
              <RestartDaemonButton
                label="Restart background service"
                onRestarted={() => {
                  setLoading(true);
                  void load();
                }}
              />
<<<<<<< Updated upstream
            </div>
          ) : isEmpty ? (
            // bdac.63: proper empty state instead of "(no log entries)" sentinel.
            // Centered muted message matching the app's empty-state pattern.
            <div className="flex h-full items-center justify-center">
              <p className="text-[13px] text-ide-faint" data-testid="log-empty-state">
                No log entries yet
              </p>
||||||| Stash base
=======
>>>>>>> Stashed changes
            </div>
          ) : (
            // Scrollable log area — no extra wrapper border; the rows live directly
            // on the content panel surface (one cohesive glass surface, no box-in-box).
            // select-text preserved so users can still copy lines.
            <div
              ref={scrollRef}
              className="h-full w-full overflow-auto select-text"
            >
              {lines.map((line, i) => {
                const level = levelOf(line);
                const isLast = i === lastLineIdx;
                return (
                  // Each row is a .mono-line glass pill — hover accent border via
                  // Tailwind group and inline transition (mirrors source.html §870-896).
                  // border-l-2 gives a per-level accent stripe (error/warn only).
                  <div
                    key={i}
                    className={[
                      "list-item-in",
                      "group flex items-start gap-2",
                      "rounded border border-ide-border bg-ide-hover",
                      "px-2 py-1 font-mono text-[11px] leading-relaxed",
                      "mb-1 last:mb-0",
                      "border-l-2",
                      LEVEL_BORDER[level],
                      LEVEL_CLASS[level],
                      // Hover: accent-tinted border + subtle bg fill (mirrors .mono-line:hover)
                      "transition-[border-color,background] duration-200 ease-out",
                      "hover:border-ide-accent/40 hover:bg-ide-accent/5",
                      "cursor-default whitespace-pre-wrap break-words",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    style={{ animationDelay: `${Math.min(i * 8, 200)}ms` }}
                  >
                    <code className="flex-1 overflow-hidden">{line || " "}</code>
                    {/* Terminal cursor blink on the last (live) line. */}
                    {isLast && (
                      <span
                        className="cursor-blink ml-0.5 inline-block h-[1em] w-[6px] shrink-0 rounded-sm bg-current opacity-70"
                        aria-hidden="true"
                      />
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* SCRL-13: corrected copy — daemon emits plain text, not JSON. */}
        <div className="mt-2 shrink-0 border-t border-ide-border pt-2 text-[11px] text-ide-faint">
          Last {MAX_LINES} lines · Plain-text log
        </div>
      </div>
    </ViewShell>
  );
}
