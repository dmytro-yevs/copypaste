//! Clipboard/history IPC dispatch facade (split from ipc god-module, ra15.1;
//! further split into handlers_items_{read,mutate,clipboard,media,ingest,
//! paste}.rs per ADR-017 daemon-ipc track, CopyPaste-vp63.15). Verb bodies
//! now live in the sibling submodules as `handle_<verb>` methods on
//! `IpcServer`; this file keeps only the two dispatch tables
//! (`dispatch_items` / `dispatch_items_extra`) and their chain-of-
//! responsibility links.
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_items(&self, req: Request) -> Response {
        match req.method.as_str() {
            "list" => self.handle_list(req).await,
            "delete" => self.handle_delete(req).await,
            "count" => self.handle_count(req).await,
            "search" => self.handle_search(req).await,
            "copy" | "paste" => self.handle_copy_or_paste(req).await,
            "delete_all" => self.handle_delete_all(req).await,
            "stats" => self.handle_stats(req).await,
            "pin" => self.handle_pin(req).await,
            "pin_item" => self.handle_pin_item(req).await,
            "reorder_pinned" => self.handle_reorder_pinned(req).await,
            "delete_item" => self.handle_delete_item(req).await,
            "copy_item" => self.handle_copy_item(req).await,
            "get_item_image" => self.handle_get_item_image(req).await,
            "get_item_thumbnail" => self.handle_get_item_thumbnail(req).await,
            "get_item_file" => self.handle_get_item_file(req).await,
            "history_page" => self.handle_history_page(req).await,
            _ => self.dispatch_config(req).await,
        }
    }

    pub(crate) async fn dispatch_items_extra(&self, req: Request) -> Response {
        match req.method.as_str() {
            "get_app_icon" => self.handle_get_app_icon(req).await,
            "add_file_item" => self.handle_add_file_item(req).await,
            other => Response::err(req.id, format!("unknown method: {other}")),
        }
    }
}
