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
import { Dialog } from "../../lib/dialog/Dialog";
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

function FullResImage({ id, maxHeight: _maxHeight }: { id: string; maxHeight: number }) {
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
      <span>
        <span>Couldn't load image</span>
        <span>Try reopening this item.</span>
      </span>
    );
  }
  if (src === null) {
    // CopyPaste-bdac.66: "Loading…" → italic/faint consistent with other placeholder states.
    return <span>Loading…</span>;
  }
  return (
    <img src={src} alt="" />
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

  // Focus trap, Escape + backdrop dismissal, and scroll-lock now come from the
  // shared Dialog primitive (task 2.9). onClose preserves the close behavior.
  const modalTitle = isImage ? "Image preview" : isFile ? "File details" : "Text preview";

  return (
    <Dialog labelledBy="details-modal-title" onClose={onClose} className="modal--wide">
        {/* Header */}
        <div>
          <span id="details-modal-title" className="modal__t">
            {modalTitle}
          </span>
          <button
            type="button"
            className="iconbtn"
            aria-label="Close"
            onClick={onClose}
          />
        </div>

        {/* Body */}
        <div>
          {isImage ? (
            // Full-res for detail modal — one image at a time, no shared cache.
            <FullResImage id={entry.id} maxHeight={600} />
          ) : isFile ? (
            // File detail: show a full-width FileChip (with Save As + Copy actions)
            // plus metadata rows. No raw binary preview — that's not useful.
            <div>
              <FileChip
                id={entry.id}
                filename={parseFilename(entry.preview)}
                mime="application/octet-stream"
              />
              <table>
                <tbody>
                  <tr>
                    <td>Name</td>
                    <td>{parseFilename(entry.preview)}</td>
                  </tr>
                  <tr>
                    <td>Type</td>
                    <td>{entry.content_type}</td>
                  </tr>
                  <tr>
                    <td>Copied</td>
                    <td>{new Date(entry.wall_time).toLocaleString()}</td>
                  </tr>
                  {entry.app_bundle_id && (
                    <tr>
                      <td>Source</td>
                      <td>{entry.app_bundle_id}</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          ) : (
            <div>
              {/* SCRH-8 DOM-leak fix: the real plaintext MUST NOT appear in the DOM
                  while blurred. We render the placeholder string instead, and only
                  swap in entry.preview after an explicit reveal action. CSS blur
                  alone is insufficient — screen readers, devtools, and clipboard
                  scanners all read raw text nodes regardless of visual styling. */}
              <pre>
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
                  onClick={() => setRevealed(true)}
                  title="Click to reveal sensitive content"
                >
                  {/* bdac.69: primary label aligned with Android cd_sensitive_item
                      ("Sensitive content — preview hidden"). macOS adds the platform-
                      specific action hint inline so users know a click reveals it. */}
                  <span>
                    Sensitive — preview hidden · click to reveal
                  </span>
                </div>
              )}
              {/* When warnings are off and still blurred, make the pre itself clickable to reveal. */}
              {blurred && !showSensitiveWarnings && (
                <div
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
        <div>
          {!isFile && <span>{entry.content_type}</span>}
          {entry.app_bundle_id && !isFile && (
            // Show the human-readable app label; raw bundle ID is available via title tooltip.
            <span title={entry.app_bundle_id}>
              {sourceAppLabel(entry.app_bundle_id) || entry.app_bundle_id}
            </span>
          )}
          {!isFile && (
            <span>{new Date(entry.wall_time).toLocaleString()}</span>
          )}
        </div>
    </Dialog>
  );
}
