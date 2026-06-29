/**
 * DetailsModal — M10: full preview for text and image clip entries.
 * Extracted from HistoryView.tsx (CopyPaste-g06m.13 refactor).
 *
 * Includes FullResImage and its 2-entry LRU cache (s7ia C1):
 * re-opening the same modal or flipping between two images doesn't re-fetch
 * the ~40 MB bitmap each time. The cache lives at module scope (not in React
 * state) so it survives unmount/remount cycles across modal opens.
 */
import { useState, useRef, useEffect } from "react";
import { useSensitiveReveal } from "../../hooks/useSensitiveReveal";
import { api, isImageType, sourceAppLabel, type HistoryEntry } from "../../lib/ipc";
import { shouldMask, maskPlaceholder } from "../../lib/masking";
import { useFocusTrap } from "../../lib/useFocusTrap";
import { FileChip } from "../../components/FileChip";

/**
 * Extract the filename from the daemon's "[file: <name>]" preview placeholder.
 * Falls back to "file" when the format doesn't match (e.g. older daemon builds).
 * Duplicated from HistoryView helpers — both DetailsModal and HistoryRow need it.
 */
function parseFilename(preview: string): string {
  const m = preview.match(/^\[file:\s*(.+)\]$/);
  return m ? m[1].trim() : preview || "file";
}

// ---------------------------------------------------------------------------
// FullResImage — fetches the FULL-RESOLUTION image for the detail modal.
// Unlike ImageThumb (which fetches the small thumbnail), this always calls
// getItemImage so the detail view shows the original quality image.
//
// s7ia C1: 2-entry LRU module-level cache so re-opening the same modal or
// flipping between two images doesn't re-fetch + re-decode the ~40 MB bitmap
// each time. The cache lives at module scope (not in React state) so it
// survives unmount/remount cycles across modal opens.
// ---------------------------------------------------------------------------

/** Simple 2-entry LRU cache for full-resolution image data URIs. */
const fullResCache = new Map<string, string>();
const FULL_RES_CACHE_MAX = 2;

function fullResCacheGet(id: string): string | undefined {
  const val = fullResCache.get(id);
  if (val === undefined) return undefined;
  // Touch: re-insert at tail (most-recently-used).
  fullResCache.delete(id);
  fullResCache.set(id, val);
  return val;
}

function fullResCacheSet(id: string, uri: string): void {
  fullResCache.delete(id); // remove first to update position
  fullResCache.set(id, uri);
  // Evict LRU entry when over capacity.
  if (fullResCache.size > FULL_RES_CACHE_MAX) {
    const oldest = fullResCache.keys().next().value;
    if (oldest !== undefined) fullResCache.delete(oldest);
  }
}

function FullResImage({ id, maxHeight }: { id: string; maxHeight: number }) {
  const [src, setSrc] = useState<string | null>(() => fullResCacheGet(id) ?? null);
  const [failed, setFailed] = useState(false);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    // Check the cache first — avoids the ~40MB re-decode on re-open.
    const cached = fullResCacheGet(id);
    if (cached !== undefined) {
      setSrc(cached);
      return () => { mountedRef.current = false; };
    }
    setSrc(null);
    setFailed(false);
    api
      .getItemImage(id)
      .then(({ data_uri }) => {
        if (!mountedRef.current) return;
        fullResCacheSet(id, data_uri);
        setSrc(data_uri);
      })
      .catch(() => {
        if (!mountedRef.current) return;
        setFailed(true);
      });
    return () => { mountedRef.current = false; };
  }, [id]);

  if (failed) {
    // CopyPaste-bdac.66: old placeholder removed → consistent error pattern with sub-hint.
    // Matches other empty/error states: italic+faint primary, ghost sub-line.
    return (
      <span className="flex flex-col items-center gap-1 py-4 text-center">
        <span className="text-[12px] text-ide-dim italic">Couldn't load image</span>
        <span className="text-[11px] text-ide-faint">Try reopening this item.</span>
      </span>
    );
  }
  if (src === null) {
    // CopyPaste-bdac.66: "Loading…" → italic/faint consistent with other placeholder states.
    return <span className="text-[12px] text-ide-faint italic">Loading…</span>;
  }
  return (
    <img
      src={src}
      alt=""
      style={{
        maxWidth: "100%",
        maxHeight: maxHeight,
        width: "auto",
        height: "auto",
        objectFit: "contain",
        imageRendering: "auto",
        display: "block",
        borderRadius: 2,
      }}
    />
  );
}

// ---------------------------------------------------------------------------
// M10: DetailsModal — full preview for text and image clip entries
// ---------------------------------------------------------------------------

export function DetailsModal({
  entry,
  maskSensitive,
  showSensitiveWarnings,
  onClose,
}: {
  entry: HistoryEntry;
  maskSensitive: boolean;
  /** n9gp (PG-34): when false, the "Sensitive — preview hidden · click to reveal" overlay is skipped;
   *  clicking the blurred pre directly unblurs without an extra confirmation step. */
  showSensitiveWarnings: boolean;
  onClose: () => void;
}) {
  const isImage = isImageType(entry.content_type);
  const isFile = entry.content_type === "file";

  // Per-modal reveal: user must click "Reveal" to see sensitive plaintext.
  // #17: useSensitiveReveal encapsulates revealed state + SCRH-7 auto-blur on window blur.
  const { revealed, setRevealed } = useSensitiveReveal({
    isSensitive: entry.is_sensitive,
    maskSensitive,
  });
  const blurred = shouldMask(entry, maskSensitive) && !revealed;

  // Focus trap — traps Tab/Shift+Tab inside the dialog panel and restores focus on close.
  const modalRef = useRef<HTMLDivElement>(null);
  useFocusTrap(modalRef);

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  const modalTitle = isImage ? "Image preview" : isFile ? "File details" : "Text preview";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="details-modal-title"
      className="fixed inset-0 z-50 flex items-center justify-center"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
      // Modal scrim: uses --ide-scrim token so dark theme (55%) and light theme (35%)
      // apply the correct overlay opacity — not surface-glass (CopyPaste-5917.42 / 5917.106).
      style={{ background: "var(--scrim)", backdropFilter: "blur(4px)" }}
    >
      <div
        ref={modalRef}
        // surface-glass-strong = floating frosted-glass dialog: the dimmed,
        // blurred content behind the scrim shows through the translucent panel.
        className="surface-glass-strong relative flex max-h-[80vh] w-[480px] max-w-[90vw] flex-col overflow-hidden rounded-xl shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex shrink-0 items-center justify-between border-b border-ide-border px-4 py-2.5">
          <span id="details-modal-title" className="text-[13px] font-medium text-ide-text">
            {modalTitle}
          </span>
          <button
            type="button"
            aria-label="Close"
            onClick={onClose}
            className="flex h-6 w-6 items-center justify-center rounded hover:bg-ide-hover text-ide-dim"
          >
            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
              <path d="M3 3l10 10M13 3 3 13" />
            </svg>
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-auto p-4">
          {isImage ? (
            // Full-res for detail modal — one image at a time, no shared cache.
            <FullResImage id={entry.id} maxHeight={600} />
          ) : isFile ? (
            // File detail: show a full-width FileChip (with Save As + Copy actions)
            // plus metadata rows. No raw binary preview — that's not useful.
            <div className="flex flex-col gap-3">
              <FileChip
                id={entry.id}
                filename={parseFilename(entry.preview)}
                mime="application/octet-stream"
              />
              <table className="text-[12px] text-ide-dim w-full border-collapse">
                <tbody>
                  <tr>
                    <td className="py-0.5 pr-3 text-ide-faint font-medium w-20">Name</td>
                    <td className="py-0.5 break-all">{parseFilename(entry.preview)}</td>
                  </tr>
                  <tr>
                    <td className="py-0.5 pr-3 text-ide-faint font-medium">Type</td>
                    <td className="py-0.5">{entry.content_type}</td>
                  </tr>
                  <tr>
                    <td className="py-0.5 pr-3 text-ide-faint font-medium">Copied</td>
                    <td className="py-0.5">{new Date(entry.wall_time).toLocaleString()}</td>
                  </tr>
                  {entry.app_bundle_id && (
                    <tr>
                      <td className="py-0.5 pr-3 text-ide-faint font-medium">Source</td>
                      <td className="py-0.5">{entry.app_bundle_id}</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="relative">
              {/* SCRH-8 DOM-leak fix: the real plaintext MUST NOT appear in the DOM
                  while blurred. We render the placeholder string instead, and only
                  swap in entry.preview after an explicit reveal action. CSS blur
                  alone is insufficient — screen readers, devtools, and clipboard
                  scanners all read raw text nodes regardless of visual styling. */}
              <pre
                className="whitespace-pre-wrap break-words text-[13px] text-ide-text font-mono leading-relaxed select-text"
                style={{
                  userSelect: blurred ? "none" : "text",
                  // No CSS blur on the placeholder — it would look odd and is
                  // redundant since the real text is not present anyway.
                  opacity: blurred ? 0.55 : 1,
                  fontStyle: blurred ? "italic" : "normal",
                  transition: "opacity 0.15s ease",
                }}
              >
                {blurred ? maskPlaceholder() : entry.preview}
              </pre>
              {/* n9gp (PG-34): show the confirmation overlay only when
                  showSensitiveWarnings is true (default). When false, the user
                  can click the placeholder pre directly to reveal without an extra
                  confirmation step (matches Android show_sensitive_warnings=false). */}
              {blurred && showSensitiveWarnings && (
                // Reveal overlay — sits on top of the placeholder so the user
                // gets a clear affordance without needing to read the italic hint.
                <div
                  className="absolute inset-0 flex items-center justify-center"
                  style={{ cursor: "pointer" }}
                  onClick={() => setRevealed(true)}
                  title="Click to reveal sensitive content"
                >
                  {/* bdac.69: primary label aligned with Android cd_sensitive_item
                      ("Sensitive content — preview hidden"). macOS adds the platform-
                      specific action hint inline so users know a click reveals it. */}
                  <span className="rounded-md border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-dim shadow">
                    Sensitive — preview hidden · click to reveal
                  </span>
                </div>
              )}
              {/* When warnings are off and still blurred, make the pre itself clickable to reveal. */}
              {blurred && !showSensitiveWarnings && (
                <div
                  className="absolute inset-0"
                  style={{ cursor: "pointer" }}
                  onClick={() => setRevealed(true)}
                  title="Click to reveal sensitive content"
                />
              )}
            </div>
          )}
        </div>

        {/* Footer: metadata.
            For file entries, Type and Copied are already in the table body — omit
            them here to avoid duplication. For image/text entries the footer is
            the only metadata row, so show content_type + source app + timestamp. */}
        <div className="shrink-0 border-t border-ide-border px-4 py-2 text-[11px] text-ide-faint flex items-center gap-3">
          {!isFile && <span>{entry.content_type}</span>}
          {entry.app_bundle_id && !isFile && (
            // Show the human-readable app label; raw bundle ID is available via title tooltip.
            <span title={entry.app_bundle_id}>
              {sourceAppLabel(entry.app_bundle_id) || entry.app_bundle_id}
            </span>
          )}
          {!isFile && (
            <span className="ml-auto">{new Date(entry.wall_time).toLocaleString()}</span>
          )}
        </div>
      </div>
    </div>
  );
}
