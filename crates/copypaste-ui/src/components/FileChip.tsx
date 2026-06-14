import { useRef, useState } from "react";
import { api } from "../lib/ipc";

// ---------------------------------------------------------------------------
// formatBytes — human-readable file size (exported for unit tests)
// ---------------------------------------------------------------------------

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

// ---------------------------------------------------------------------------
// File-type icon — a generic document SVG styled in amber/orange to distinguish
// it from image (purple) and text (blue) content types.
// ---------------------------------------------------------------------------

function FileIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      width="14"
      height="14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      {/* Document outline with folded corner */}
      <path d="M9.5 1.5H3.5a1 1 0 0 0-1 1v11a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1V5.5L9.5 1.5Z" />
      <path d="M9.5 1.5v4h4" />
      {/* Three ruled lines */}
      <line x1="5" y1="8" x2="11" y2="8" />
      <line x1="5" y1="10.5" x2="11" y2="10.5" />
      <line x1="5" y1="5.5" x2="8" y2="5.5" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Trigger a browser download for the given base64-encoded file data.
// Uses a temporary <a download> element — works in Tauri's webview without
// needing plugin-dialog or plugin-fs (neither is installed in this project).
// ---------------------------------------------------------------------------

function triggerDownload(filename: string, mime: string, data_b64: string): void {
  const byteString = atob(data_b64);
  const bytes = new Uint8Array(byteString.length);
  for (let i = 0; i < byteString.length; i++) {
    bytes[i] = byteString.charCodeAt(i);
  }
  const blob = new Blob([bytes], { type: mime });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
  // Revoke after a short delay so the download can start before the blob is freed.
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

// ---------------------------------------------------------------------------
// FileChip props
// ---------------------------------------------------------------------------

export interface FileChipProps {
  /** Clipboard item ID used for IPC calls. */
  id: string;
  /** Original filename to display and use as the download name. */
  filename: string;
  /** MIME type — used for the download blob and optional display. */
  mime: string;
  /**
   * Optional pre-known size in bytes (from the history entry or a prior fetch).
   * When provided, size is shown immediately without a fetch. When absent,
   * the size is shown only after a successful Save As fetch.
   */
  sizeBytes?: number;
  /** Called after a successful copy_item IPC so the parent can show a toast. */
  onCopied?: () => void;
}

// ---------------------------------------------------------------------------
// FileChip — renders a file row chip with:
//   - file icon + filename
//   - human-readable size (if known, or lazily after fetch)
//   - "Save As…" button: fetches full file data and triggers a browser download
//   - "Copy" button: tells the daemon to copy the file back to the pasteboard
// ---------------------------------------------------------------------------

export function FileChip({ id, filename, mime, sizeBytes, onCopied }: FileChipProps) {
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [opening, setOpening] = useState(false);
  const [openError, setOpenError] = useState<string | null>(null);
  const [copying, setCopying] = useState(false);
  const [resolvedSize, setResolvedSize] = useState<number | null>(sizeBytes ?? null);
  const mountedRef = useRef(true);

  // Clear mounted flag on unmount to avoid setting state on an unmounted component.
  // useEffect cleanup runs synchronously on unmount.
  const mountedRefCallback = (node: HTMLSpanElement | null) => {
    if (node === null) {
      mountedRef.current = false;
    } else {
      mountedRef.current = true;
    }
  };

  const handleSaveAs = async () => {
    if (saving) return;
    setSaving(true);
    setSaveError(null);
    try {
      const result = await api.getItemFile(id);
      if (!mountedRef.current) return;
      // Derive size from the decoded data if we didn't have it yet.
      if (resolvedSize === null) {
        const byteLen = Math.floor((result.data_b64.length * 3) / 4);
        setResolvedSize(byteLen);
      }
      triggerDownload(result.filename || filename, result.mime || mime, result.data_b64);
    } catch (err) {
      if (!mountedRef.current) return;
      const msg = err instanceof Error ? err.message : String(err);
      setSaveError(`Save failed: ${msg}`);
    } finally {
      if (mountedRef.current) setSaving(false);
    }
  };

  // Open the file with the OS default application by writing it to a temp path.
  // Uses the native open_item_file Tauri command (macOS: /usr/bin/open, Linux: xdg-open).
  const handleOpen = async () => {
    if (opening) return;
    setOpening(true);
    setOpenError(null);
    try {
      await api.openItemFile(id);
    } catch (err) {
      if (!mountedRef.current) return;
      const msg = err instanceof Error ? err.message : String(err);
      setOpenError(`Open failed: ${msg}`);
    } finally {
      if (mountedRef.current) setOpening(false);
    }
  };

  const handleCopy = async () => {
    if (copying) return;
    setCopying(true);
    try {
      await api.copyItem(id);
      if (!mountedRef.current) return;
      onCopied?.();
    } catch {
      // Copy errors are best-effort; the parent can also show a toast via onCopied.
    } finally {
      if (mountedRef.current) setCopying(false);
    }
  };

  return (
    <span
      ref={mountedRefCallback}
      className="inline-flex items-center gap-2 rounded border border-ide-divider/60 bg-ide-elevated/60 px-2 py-1"
      style={{ maxWidth: "100%" }}
    >
      {/* File icon — amber/orange to distinguish from image/text */}
      <FileIcon className="shrink-0 text-ide-warning" />

      {/* Filename + optional size */}
      <span className="flex min-w-0 flex-col">
        <span
          className="truncate text-[12px] text-ide-text leading-snug"
          title={filename}
        >
          {filename}
        </span>
        {resolvedSize !== null && (
          <span className="text-[10px] text-ide-faint leading-snug">
            {formatBytes(resolvedSize)}
          </span>
        )}
      </span>

      {/* Error message when Save As or Open fails */}
      {saveError !== null && (
        <span className="text-[11px] text-ide-danger shrink-0">{saveError}</span>
      )}
      {openError !== null && (
        <span className="text-[11px] text-ide-danger shrink-0">{openError}</span>
      )}

      {/* Action buttons */}
      <span className="ml-auto flex shrink-0 items-center gap-1">
        {/* Open — write to temp file and open with OS default app (no save dialog) */}
        <button
          type="button"
          aria-label="Open"
          title="Open with default app"
          disabled={opening}
          onClick={() => void handleOpen()}
          className="flex items-center gap-1 rounded border border-ide-border bg-ide-elevated px-1.5 py-0.5 text-[11px] text-ide-dim hover:bg-ide-hover hover:text-ide-text disabled:opacity-50"
        >
          {opening ? "Opening…" : "Open"}
        </button>
        <button
          type="button"
          aria-label="Save As"
          title="Save As…"
          disabled={saving}
          onClick={() => void handleSaveAs()}
          className="flex items-center gap-1 rounded border border-ide-border bg-ide-elevated px-1.5 py-0.5 text-[11px] text-ide-dim hover:bg-ide-hover hover:text-ide-text disabled:opacity-50"
        >
          {saving ? "Saving…" : "Save As…"}
        </button>
        <button
          type="button"
          aria-label="Copy"
          title="Copy to clipboard"
          disabled={copying}
          onClick={() => void handleCopy()}
          className="flex items-center gap-1 rounded border border-ide-border bg-ide-elevated px-1.5 py-0.5 text-[11px] text-ide-dim hover:bg-ide-hover hover:text-ide-text disabled:opacity-50"
        >
          {copying ? "…" : "Copy"}
        </button>
      </span>
    </span>
  );
}
