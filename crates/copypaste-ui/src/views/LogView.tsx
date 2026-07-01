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

  // Actions slot rendered into the ViewShell header (Refresh + Export buttons).
  const headerActions = (
    <div>
      <button
        onClick={() => {
          setLoading(true);
          void load();
        }}
      >
        Refresh
      </button>
      <button
        onClick={handleExport}
        disabled={isEmpty || !content}
      >
        Export
      </button>
    </div>
  );

  return (
    // SCRL-11: use shared ViewShell for consistent header + drag-region + glass
    // surface. The redundant inner surface-card header is removed (fixes SCRL-3).
    <ViewShell title="Daemon Logs" actions={headerActions}>
      <div>
        {/* Path subtitle shown below the ViewShell title, inside the content panel. */}
        {logPath && (
          <p title={relativizeLogPath(logPath)}>
            {relativizeLogPath(logPath)}
          </p>
        )}

        {/* Content */}
        <div>
          {loading ? (
            <div>
              <span aria-label="Loading logs…" />
            </div>
          ) : error ? (
            /* bdac.96: RestartDaemonButton added to error state so users can recover
               without navigating away — consistent with HistoryView / DevicesView. */
            <div>
              <p>{error}</p>
              <RestartDaemonButton
                label="Restart background service"
                onRestarted={() => {
                  setLoading(true);
                  void load();
                }}
              />
            </div>
          ) : isEmpty ? (
            // bdac.63: proper empty state instead of "(no log entries)" sentinel.
            <div>
              <p data-testid="log-empty-state">
                No log entries yet
              </p>
            </div>
          ) : (
            // Scrollable log area — select-text preserved so users can still copy lines.
            <div ref={scrollRef}>
              {lines.map((line, i) => {
                const level = levelOf(line);
                return (
                  // data-level carries the parsed log level as data, not styling.
                  <div key={i} data-level={level}>
                    <code>{line || " "}</code>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* SCRL-13: corrected copy — daemon emits plain text, not JSON. */}
        <div>
          Last {MAX_LINES} lines · Plain-text log
        </div>
      </div>
    </ViewShell>
  );
}
