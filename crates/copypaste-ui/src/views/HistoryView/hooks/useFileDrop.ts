/**
 * useFileDrop — file picker (D2) and OS drag-drop (D3) hook for HistoryView.
 *
 * Extracted from HistoryView.tsx (CopyPaste-g06m.34 refactor).
 * Owns: fileDragOver, fileInputRef, handleFileInputChange, and the Tauri
 *       onDragDropEvent subscription.
 *
 * Requires load + showToast callbacks from the parent so IPC calls stay
 * routed through the existing ipc client (no new direct core access).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { api, friendlyIpcError } from "../../../lib/ipc";
import type { ToastKind } from "../../../components/Toast";

// getCurrentWebview is only available inside the Tauri runtime. Import it
// lazily so the module can load in a plain browser without crashing at
// import time (the symbol would be undefined / the package would throw).
// We feature-detect at call-site via `window.__TAURI_INTERNALS__`.
let _getCurrentWebview: typeof import("@tauri-apps/api/webview").getCurrentWebview | null = null;
if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
  void import("@tauri-apps/api/webview").then((m) => {
    _getCurrentWebview = m.getCurrentWebview;
  });
}

export function useFileDrop(
  load: (silent?: boolean) => Promise<void>,
  showToast: (message: string, kind: ToastKind, durationMs?: number) => void,
) {
  // D3: OS-level file drag-drop state — true while files are hovering over the window
  const [fileDragOver, setFileDragOver] = useState(false);

  // Hidden file-input ref for D2 (browser-picker path)
  const fileInputRef = useRef<HTMLInputElement>(null);

  // -------------------------------------------------------------------------
  // D2: File picker — read the chosen file via the browser File API and send
  // to the daemon. No Rust-side file dialog needed; <input type="file"> gives
  // us the bytes directly so we can base64-encode and call add_file_item.
  // -------------------------------------------------------------------------

  const handleFileInputChange = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = Array.from(e.target.files ?? []);
      if (files.length === 0) return;
      // Reset the input so the same file can be picked again if needed.
      e.target.value = "";

      for (const file of files) {
        try {
          const bytes = new Uint8Array(await file.arrayBuffer());
          await api.addFileItem(bytes, file.name, file.type || "application/octet-stream");
          showToast(`Added "${file.name}"`, "success");
        } catch (err) {
          // ERR-2: friendlyIpcError never leaks socket paths or raw transport strings.
          console.error("[HistoryView] add file error:", err);
          const msg = friendlyIpcError(err);
          showToast(`Failed to add "${file.name}": ${msg}`, "error");
        }
      }
      void load(true);
    },
    [load, showToast]
  );

  // -------------------------------------------------------------------------
  // D3: OS file drag-drop — subscribe to Tauri's webview onDragDropEvent.
  // On 'enter': show drop-zone overlay. On 'drop': ingest each file via
  // add_file_item. On 'leave'/'cancel': hide overlay.
  // NOTE: dragDropEnabled must be true in tauri.conf.json (already set).
  // -------------------------------------------------------------------------

  useEffect(() => {
    // Tauri-only: OS file drag-drop via the webview's onDragDropEvent API.
    // In a plain browser `_getCurrentWebview` is null (set only when
    // window.__TAURI_INTERNALS__ exists), so we skip the subscription entirely.
    // The browser <input type="file"> path (D2) still works without Tauri.
    if (_getCurrentWebview === null) return;

    let unlisten: (() => void) | null = null;
    let cancelled = false;

    void _getCurrentWebview()
      .onDragDropEvent((event) => {
        if (cancelled) return;
        const { type } = event.payload;

        if (type === "enter") {
          setFileDragOver(true);
        } else if (type === "leave") {
          setFileDragOver(false);
        } else if (type === "drop") {
          setFileDragOver(false);
          const paths = "paths" in event.payload ? (event.payload.paths as string[]) : [];
          if (paths.length === 0) return;

          void (async () => {
            let added = 0;
            let failed = 0;
            for (const p of paths) {
              try {
                // Read via fetch with a file:// URL — works inside Tauri webview.
                const resp = await fetch(`file://${p}`);
                if (!resp.ok) throw new Error(`fetch failed: ${resp.status}`);
                const buf = await resp.arrayBuffer();
                const bytes = new Uint8Array(buf);
                const filename = p.split("/").pop() ?? "file";
                // Infer MIME from the content-type header (best-effort).
                const mime =
                  resp.headers.get("content-type")?.split(";")[0]?.trim() ||
                  "application/octet-stream";
                await api.addFileItem(bytes, filename, mime);
                added++;
              } catch (err) {
                failed++;
                // ERR-2: friendlyIpcError never leaks socket paths or raw transport strings.
                console.error("[HistoryView] drag-drop file error:", err);
                const msg = friendlyIpcError(err);
                showToast(`Drop failed for "${p.split("/").pop()}": ${msg}`, "error");
              }
            }
            if (added > 0) {
              showToast(
                `Added ${added} file${added === 1 ? "" : "s"}${failed > 0 ? ` (${failed} failed)` : ""}`,
                "success"
              );
              void load(true);
            }
          })();
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {
        // Best-effort — drag-drop is a convenience, never block on its failure.
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [load, showToast]);

  return {
    fileDragOver,
    fileInputRef,
    handleFileInputChange,
  };
}
