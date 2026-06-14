import { useState, useEffect, useCallback, useRef } from "react";
import { readLogs, logDirPath } from "../lib/ipc";

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
      setError(err instanceof Error ? err.message : String(err));
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

  return (
    // surface-glass on the frame; the opaque bg-ide-bg was DEFEATING the glass —
    // removed so the aurora canvas blurs through.
    <div className="surface-glass flex h-full flex-col">
      {/* Header — glass too, so it reads as a layered material, not an opaque bar. */}
      <div className="surface-card flex shrink-0 items-center justify-between border-b border-ide-border px-4 py-3">
        <div>
          <h2 className="text-[13px] font-medium text-ide-text">Daemon Logs</h2>
          {logPath && (
            <p className="mt-0.5 text-[11px] text-ide-faint" title={logPath}>
              {logPath}
            </p>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => { setLoading(true); void load(); }}
            className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-dim hover:bg-ide-raised hover:text-ide-text shadow-ide-xs"
          >
            Refresh
          </button>
          <button
            onClick={handleExport}
            disabled={!content || content === "(no log entries)"}
            className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-dim hover:bg-ide-raised hover:text-ide-text shadow-ide-xs disabled:opacity-40"
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
          // audit P2: colorized, per-line log view (replaces the flat textarea).
          // Selectable text is preserved (select-text) so users can still copy.
          <div
            ref={scrollRef}
            className="h-full w-full overflow-auto rounded-ide border border-ide-border bg-ide-raised p-3 font-mono text-[11px] leading-relaxed select-text"
          >
            {content.split("\n").map((line, i) => (
              <div
                key={i}
                className={`whitespace-pre-wrap break-words ${LEVEL_CLASS[levelOf(line)]}`}
              >
                {line || " "}
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="shrink-0 border-t border-ide-border px-4 py-2 text-[11px] text-ide-faint">
        Last {MAX_LINES} lines · Daemon log (JSON format)
      </div>
    </div>
  );
}
