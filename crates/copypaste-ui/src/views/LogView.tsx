import { useState, useEffect, useCallback, useRef } from "react";
import { Download, RefreshCw, Search } from "lucide-react";
import { readLogs, logDirPath, IpcError } from "../lib/ipc";
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
 * Map the internal LogLevel to the design system's `.lvl` severity modifier
 * (`ok`/`info`/`warn`/`err` — shell.css `.logline .lvl`). There is no
 * dedicated DEBUG/TRACE swatch in the design system, so it folds into `info`
 * (still a neutral/subdued tone relative to WARN/ERROR); the displayed label
 * text still shows the real level (levelOf's return value), only the colour
 * class is collapsed.
 */
function lvlClass(level: LogLevel): "ok" | "info" | "warn" | "err" {
  switch (level) {
    case "error":
      return "err";
    case "warn":
      return "warn";
    case "info":
    case "debug":
    default:
      return "info";
  }
}

// Matches tracing_subscriber's compact formatter: "<timestamp>  <LEVEL> <target>: <message>".
const LOG_LINE_RE = /^(\S+)\s+(TRACE|DEBUG|INFO|WARN|ERROR)\s+(.*)$/;

/**
 * Split one raw daemon log line into a `.t` timestamp + `.m` message for the
 * `.logline` layout (shell.css). Falls back to an empty timestamp and the
 * full line as the message for anything that doesn't match (e.g. a wrapped
 * continuation line) — no log content is ever dropped.
 */
function splitLogLine(line: string): { t: string; m: string } {
  const match = LOG_LINE_RE.exec(line);
  if (match) {
    return { t: match[1], m: match[3] };
  }
  return { t: "", m: line };
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

export function LogContent() {
  const [content, setContent] = useState<string>("");
  // bdac.63: track empty state as a boolean, not a string sentinel.
  // Previously setContent("(no log entries)") tied display text to logic.
  const [isEmpty, setIsEmpty] = useState(false);
  const [logPath, setLogPath] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  // Slice 5 (CopyPaste-g27b.12): client-side substring filter over the
  // already-loaded lines, wired to the `.field` search box in the header.
  const [filter, setFilter] = useState("");
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
  const trimmedFilter = filter.trim().toLowerCase();
  const visibleLines = trimmedFilter
    ? lines.filter((line) => line.toLowerCase().includes(trimmedFilter))
    : lines;

  // Actions slot rendered into the ViewShell header (filter field + Refresh +
  // Export buttons). Wrapped locally in `.srow__c` (shell.css "flex cluster of
  // controls") so this view's own row lays out correctly — kept local to
  // LogView rather than changing ViewShell's shared actions wrapper, which
  // would also reflow the other (still-unwired) views' action rows.
  const headerActions = (
    <div className="srow__c">
      <div className="field">
        <Search aria-hidden="true" />
        <input
          type="search"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="Filter logs…"
          aria-label="Filter logs"
        />
      </div>
      <button
        type="button"
        className="btn sm btn--secondary"
        onClick={() => {
          setLoading(true);
          void load();
        }}
      >
        <RefreshCw aria-hidden="true" />
        Refresh
      </button>
      <button
        type="button"
        className="btn sm btn--secondary"
        onClick={handleExport}
        disabled={isEmpty || !content}
      >
        <Download aria-hidden="true" />
        Export
      </button>
    </div>
  );

  return (
    // SCRL-11: use shared ViewShell for consistent header + drag-region + glass
    // surface. The redundant inner surface-card header is removed (fixes SCRL-3).
    <div className="fill-col">
      <div className="logs-toolbar">{headerActions}</div>
      {logPath && (
          <p className="logs-path" title={relativizeLogPath(logPath)}>
            {relativizeLogPath(logPath)}
          </p>
        )}

        {/* Content */}
        <div className="fill-col">
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
            <div className="logs" ref={scrollRef}>
              {visibleLines.map((line, i) => {
                const level = levelOf(line);
                const { t, m } = splitLogLine(line);
                return (
                  // data-level carries the parsed log level as data, not styling.
                  <div key={i} className="logline" data-level={level}>
                    {t && <span className="t">{t}</span>}
                    <span className={`lvl ${lvlClass(level)}`}>{level}</span>
                    <span className="m">
                      <code>{m || " "}</code>
                    </span>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* SCRL-13: corrected copy — daemon emits plain text, not JSON. */}
        <div className="logs-foot">
          Last {MAX_LINES} lines · Plain-text log
        </div>
    </div>
  );
}
