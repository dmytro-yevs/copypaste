import { useState, useEffect, useCallback, useRef } from "react";
import { readLogs, logDirPath, IpcError } from "../lib/ipc";

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
      setContent(logs || "(no log entries)");
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

  return (
    // surface-glass on the frame; the opaque bg-ide-bg was DEFEATING the glass —
    // removed so the aurora canvas blurs through.
    <div className="surface-glass flex h-full flex-col">
      {/* Header — glass too, so it reads as a layered material, not an opaque bar. */}
      <div className="surface-card reveal-up flex shrink-0 items-center justify-between border-b border-ide-border px-4 py-3">
        <div>
          <h2 className="text-[13px] font-medium text-ide-text">Daemon Logs</h2>
          {logPath && (
            // title keeps the full absolute path for power-users / clipboard.
            // Display text is ~-relativized so the username never leaks
            // in screen recordings or screenshots (CopyPaste-2b3i).
            <p className="mt-0.5 text-[11px] text-ide-faint" title={logPath}>
              {relativizeLogPath(logPath)}
            </p>
          )}
        </div>
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
            disabled={!content || content === "(no log entries)"}
            className="border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-dim shadow-ide-xs hover:bg-ide-raised hover:text-ide-text disabled:opacity-40"
            style={{ borderRadius: "var(--skin-r-ctl)" }}
          >
            Export
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="min-h-0 flex-1 p-3">
        {loading ? (
          <div className="flex h-full items-center justify-center text-[13px] text-ide-dim">
            Loading…
          </div>
        ) : error ? (
          <div className="flex h-full items-center justify-center">
            <p className="text-[13px] text-ide-danger">{error}</p>
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
                    "rounded border border-ide-border bg-black/10",
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

      <div className="shrink-0 border-t border-ide-border px-4 py-2 text-[11px] text-ide-faint">
        Last {MAX_LINES} lines · Daemon log (JSON format)
      </div>
    </div>
  );
}
