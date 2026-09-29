//! History commands: list, search, add, copy, delete, delete_all,
//! set_pinned.
//!
//! # Naming
//!
//! Command names track `copypaste_ipc::Method` — `delete_all` for `DeleteAll`,
//! `set_pinned` for `Pin { pinned }` — rather than reading as English verbs.
//! That is deliberate: the wire enum is the single model of the contract, so a
//! name that matches it is one fewer mapping to keep straight, and
//! `crates/copypaste-ui/src/lib/ipc.ts` is already written against exactly
//! these names.
//!

use tauri::{AppHandle, Runtime, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::backend::{Backend, BackendError, SelectedBackend};
use crate::model::{UiImagePreview, UiInstalledSourceApp, UiItem, UiPage, UiSourceAppIcon};
use crate::source_app_icon::SourceAppIconCache;
use copypaste_source_app::AppIcon;

type Result<T> = std::result::Result<T, BackendError>;

const MSG_BULK_COPY_FAILED: &str = "Those items couldn't be copied to the clipboard.";

/// Format availability for a native clipboard write. An actual write can still
/// fail, for example when image bytes cannot be decoded or file metadata is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardWriteAvailability {
    Available,
    UnsupportedContentType,
    UnsupportedOnPlatform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardWriteMode {
    Original,
    PlainText,
}

#[derive(Debug, Clone, Copy)]
enum ClipboardPlatform {
    Android,
    MacOs,
    Windows,
    Other,
}

/// Report whether the compiled platform's writer accepts this history format.
/// No item id or content crosses the WebView boundary.
#[tauri::command]
pub fn clipboard_write_availability(
    content_type: String,
    mode: ClipboardWriteMode,
) -> ClipboardWriteAvailability {
    let platform = match std::env::consts::OS {
        "android" => ClipboardPlatform::Android,
        "macos" => ClipboardPlatform::MacOs,
        "windows" => ClipboardPlatform::Windows,
        _ => ClipboardPlatform::Other,
    };
    write_availability(platform, &content_type, mode)
}

fn write_availability(
    platform: ClipboardPlatform,
    content_type: &str,
    mode: ClipboardWriteMode,
) -> ClipboardWriteAvailability {
    use copypaste_ipc::{content_type, ContentClass};
    use ClipboardWriteAvailability::{Available, UnsupportedContentType, UnsupportedOnPlatform};

    if mode == ClipboardWriteMode::PlainText {
        return match content_type::classify(content_type) {
            ContentClass::Text => Available,
            ContentClass::Image | ContentClass::File | ContentClass::Other => {
                UnsupportedContentType
            }
        };
    }

    match content_type::classify(content_type) {
        // The Linux test daemon uses the text-only fake ClipboardSource.
        ContentClass::Text => Available,
        ContentClass::Image => {
            if !matches!(
                content_type,
                content_type::IMAGE_PNG | content_type::IMAGE_TIFF | "image/bmp"
            ) {
                return UnsupportedContentType;
            }
            match platform {
                ClipboardPlatform::Windows => Available,
                ClipboardPlatform::MacOs | ClipboardPlatform::Android
                    if matches!(
                        content_type,
                        content_type::IMAGE_PNG | content_type::IMAGE_TIFF
                    ) =>
                {
                    Available
                }
                _ => UnsupportedOnPlatform,
            }
        }
        ContentClass::File => match platform {
            ClipboardPlatform::MacOs | ClipboardPlatform::Android | ClipboardPlatform::Windows => {
                Available
            }
            _ => UnsupportedOnPlatform,
        },
        ContentClass::Other => UnsupportedContentType,
    }
}

/// Most recent items, newest first; pinned ahead of unpinned.
///
/// Returns a page rather than a bare array so the count of rows that would not
/// decrypt travels with them (parity finding 17) and so the marker for the next
/// page does.
///
/// `cursor` is the previous page's `next_cursor`, and `None` asks for the
/// first. Not an offset: the list grows at the top while it is being read, so a
/// row number taken for one page names a different boundary by the next, and
/// the second page repeats a row or skips one (B-1, `CopyPaste-8ebg.57`).
#[tauri::command]
pub async fn list(
    backend: State<'_, SelectedBackend>,
    limit: u32,
    cursor: Option<String>,
) -> Result<UiPage> {
    list_page(&*backend, limit, cursor.as_deref()).await
}

/// Load a page without making a second, unrelated round trip for its count.
///
/// A page that was already read must remain readable when a daemon restart
/// races the old count request. The live status query owns the global count;
/// [`UiPage::from`] supplies the loaded-page count for the compact Quick Paste
/// surface until its next refresh.
async fn list_page(backend: &impl Backend, limit: u32, cursor: Option<&str>) -> Result<UiPage> {
    Ok(backend.list(limit, cursor).await?.into())
}

/// Full-text search over stored history.
#[tauri::command]
pub async fn search(
    backend: State<'_, SelectedBackend>,
    query: String,
    limit: u32,
) -> Result<UiPage> {
    Ok(backend.search(&query, limit).await?.into())
}

/// Add an item to history without going through the clipboard.
#[tauri::command]
pub async fn add_item(backend: State<'_, SelectedBackend>, content: String) -> Result<UiItem> {
    if content.trim().is_empty() {
        // Rejected here as well as in the backend so the round trip is not
        // spent learning something this side already knew.
        return Err(BackendError::Invalid("There is nothing to add."));
    }
    Ok(backend.add(&content).await?.into())
}

/// Put an item's content on the system clipboard.
///
/// Takes an id and writes the stored content to the system clipboard.
#[tauri::command]
pub async fn copy_item<R: Runtime>(
    app: AppHandle<R>,
    backend: State<'_, SelectedBackend>,
    id: String,
) -> Result<UiItem> {
    let item = backend.copy(&id).await?;
    crate::shell::feedback::success(&app);
    Ok(item.into())
}

/// Put an item on the clipboard as plain text, without exposing it to the WebView.
#[tauri::command]
pub async fn copy_item_as_plain_text<R: Runtime>(
    app: AppHandle<R>,
    backend: State<'_, SelectedBackend>,
    id: String,
) -> Result<UiItem> {
    let item = backend.copy_as_plain_text(&id).await?;
    crate::shell::feedback::success(&app);
    Ok(item.into())
}

/// Return the complete body of an item.
#[tauri::command]
pub async fn get_item_body(backend: State<'_, SelectedBackend>, id: String) -> Result<String> {
    Ok(backend.get(&id).await?.content)
}

/// One clipboard write for the whole selection.
///
/// Answers how many text items actually reached the clipboard.
///
/// A row that vanished between the selection and this call fails the whole
/// command and writes nothing. The clipboard is one slot: partial content under
/// a success message is pasted in full confidence.
#[tauri::command]
pub async fn copy_items<R: Runtime>(
    app: AppHandle<R>,
    backend: State<'_, SelectedBackend>,
    ids: Vec<String>,
) -> Result<u32> {
    let (text, copied) = joined_text(&*backend, &ids).await?;
    if copied == 0 {
        return Ok(0);
    }
    app.clipboard().write_text(text).map_err(|e| {
        tracing::warn!(error = %e, "a bulk clipboard write failed");
        BackendError::Internal(MSG_BULK_COPY_FAILED.to_string())
    })?;
    crate::shell::feedback::success(&app);
    Ok(copied)
}

async fn joined_text(backend: &impl Backend, ids: &[String]) -> Result<(String, u32)> {
    let mut parts: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        let item = backend.get(id).await?;
        if copypaste_ipc::content_type::is_binary(&item.content_type) {
            continue;
        }
        parts.push(item.content);
    }
    let copied = u32::try_from(parts.len()).unwrap_or(u32::MAX);
    Ok((parts.join("\n"), copied))
}

/// A lazy thumbnail for an image row.
#[tauri::command]
pub async fn get_image_preview(
    backend: State<'_, SelectedBackend>,
    id: String,
    max_edge: Option<u32>,
) -> Result<UiImagePreview> {
    Ok(backend.image_preview(&id, max_edge).await?.into())
}

/// Resolve the captured source application's icon without exposing an app path
/// to the WebView. A missing or unqueryable app is a normal fallback state.
#[cfg(not(target_os = "android"))]
#[tauri::command]
pub async fn get_source_app_icon(
    item_id: Option<String>,
    bundle_id: String,
    cache: State<'_, SourceAppIconCache>,
    backend: State<'_, SelectedBackend>,
) -> Result<Option<UiSourceAppIcon>> {
    Ok(
        source_app_icon_with_fallback(&*backend, item_id.as_deref(), || {
            cache.resolve_desktop(&bundle_id)
        })
        .await,
    )
}

async fn source_app_icon_with_fallback(
    backend: &impl Backend,
    item_id: Option<&str>,
    fallback: impl FnOnce() -> Option<UiSourceAppIcon>,
) -> Option<UiSourceAppIcon> {
    if let Some(item_id) = item_id {
        if let Ok(Some(icon)) = backend.source_app_icon(item_id).await {
            if let Some(icon) = AppIcon::from_base64(icon.png_base64, icon.width, icon.height) {
                return Some(UiSourceAppIcon::from_app_icon(icon));
            }
        }
    }
    fallback()
}

/// Installed application catalogue used by Settings → Service.
#[cfg(target_os = "android")]
#[tauri::command]
pub fn list_installed_source_apps(
    capture: State<'_, crate::capture::SelectedCapture>,
) -> Result<Vec<UiInstalledSourceApp>> {
    capture.installed_source_apps().map(|apps| {
        apps.into_iter()
            .map(|app| UiInstalledSourceApp::new(app.package_id, app.label))
            .collect()
    })
}

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub async fn list_installed_source_apps() -> Result<Vec<UiInstalledSourceApp>> {
    tauri::async_runtime::spawn_blocking(crate::installed_source_apps::list)
        .await
        .map_err(|_| BackendError::internal("Installed applications couldn't be read."))?
        .map(|apps| {
            apps.into_iter()
                .map(|app| UiInstalledSourceApp::new(app.id, app.label))
                .collect()
        })
        .map_err(|_| BackendError::internal("Installed applications couldn't be read."))
}

/// Android resolves package icons through the application PackageManager.
/// Devices or package-visibility policies that cannot query the source simply
/// retain the semantic icon already rendered by the view.
#[cfg(target_os = "android")]
#[tauri::command]
pub async fn get_source_app_icon(
    item_id: Option<String>,
    bundle_id: String,
    cache: State<'_, SourceAppIconCache>,
    capture: State<'_, crate::capture::SelectedCapture>,
    backend: State<'_, SelectedBackend>,
) -> Result<Option<UiSourceAppIcon>> {
    Ok(
        source_app_icon_with_fallback(&*backend, item_id.as_deref(), || {
            cache.resolve_with(&bundle_id, |package_id| {
                capture.source_app_icon(package_id).and_then(|icon| {
                    UiSourceAppIcon::from_base64(icon.png_base64, icon.width, icon.height)
                })
            })
        })
        .await,
    )
}

/// `true` once the backend has confirmed the row is gone. An unknown id is a
/// not-found failure, not a quiet `false`.
#[tauri::command]
pub async fn delete_item(backend: State<'_, SelectedBackend>, id: String) -> Result<bool> {
    backend.delete(&id).await?;
    Ok(true)
}

/// Delete everything, and report how many rows went.
///
/// No confirmation here: the CLI prompts because a shell has nowhere else to
/// ask, but a dialog is the frontend's to own, and putting a second gate in the
/// bridge would mean two places could disagree about whether the user meant it.
#[tauri::command]
pub async fn delete_all(backend: State<'_, SelectedBackend>, through: Option<i64>) -> Result<u64> {
    backend.clear(through).await
}

/// A marker for everything stored so far, to pass back to `delete_all`.
///
/// The frontend defers a clear behind an undo window, so it has to name the set
/// the user meant at the moment they asked rather than at the moment the call
/// finally runs.
#[tauri::command]
pub async fn history_ceiling(backend: State<'_, SelectedBackend>) -> Result<u64> {
    backend.history_ceiling().await
}

/// Pin or unpin an item, returning the updated item so the caller need not
/// re-list.
///
/// One command taking a boolean rather than a `pin` / `unpin` pair, because
/// that is the shape of `copypaste_ipc::Method::Pin` and of the frontend that
/// already calls it.
#[tauri::command]
pub async fn set_pinned(
    backend: State<'_, SelectedBackend>,
    id: String,
    pinned: bool,
) -> Result<UiItem> {
    Ok(backend.set_pinned(&id, pinned).await?.into())
}

/// Rewrite the order of the pinned section.
///
/// Takes the complete pinned list in the wanted order. Partial moves are not
/// offered: see `Backend::reorder_pinned` for why a full ordering is the only
/// shape that survives a concurrent pin.
#[tauri::command]
pub async fn reorder_pinned(backend: State<'_, SelectedBackend>, ids: Vec<String>) -> Result<()> {
    if ids.is_empty() {
        // Not an error worth a round trip, and not something to send: an empty
        // ordering would be indistinguishable from "unpin everything" to a
        // careless implementation on the other side.
        return Ok(());
    }
    backend.reorder_pinned(&ids).await
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};

    use super::*;
    use crate::backend::{testing::FakeBackend, Page};

    fn test_icon() -> AppIcon {
        let image = ImageBuffer::from_pixel(64, 64, Rgba([0x24u8, 0x65, 0xa8, 0xff]));
        let mut png = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        AppIcon::from_png(png.into_inner(), 64, 64).unwrap()
    }

    fn persisted_icon() -> copypaste_ipc::ImagePreview {
        let icon = test_icon();
        copypaste_ipc::ImagePreview {
            png_base64: icon.png_base64,
            width: icon.width,
            height: icon.height,
        }
    }

    #[tokio::test]
    async fn persisted_source_icon_precedes_the_local_resolver() {
        let backend = FakeBackend::failing().with_source_app_icon(persisted_icon());
        let fallback_called = AtomicBool::new(false);

        let icon = source_app_icon_with_fallback(&backend, Some("item-1"), || {
            fallback_called.store(true, Ordering::Relaxed);
            None
        })
        .await;

        assert!(icon.is_some());
        assert_eq!(backend.source_app_icon_calls(), 1);
        assert!(!fallback_called.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn missing_persisted_source_icon_uses_the_local_resolver() {
        let backend = FakeBackend::failing();
        let fallback_called = AtomicBool::new(false);

        let icon = source_app_icon_with_fallback(&backend, Some("item-1"), || {
            fallback_called.store(true, Ordering::Relaxed);
            Some(UiSourceAppIcon::from_app_icon(test_icon()))
        })
        .await;

        assert!(icon.is_some());
        assert_eq!(backend.source_app_icon_calls(), 1);
        assert!(fallback_called.load(Ordering::Relaxed));
    }

    #[test]
    fn clipboard_write_availability_matches_native_writers() {
        use ClipboardPlatform::{Android, MacOs, Other, Windows};
        use ClipboardWriteAvailability::{
            Available, UnsupportedContentType, UnsupportedOnPlatform,
        };
        use ClipboardWriteMode::{Original, PlainText};

        for content_type in ["text", "text/plain", "text/html"] {
            for platform in [Android, MacOs, Windows] {
                assert_eq!(
                    write_availability(platform, content_type, Original),
                    Available
                );
                assert_eq!(
                    write_availability(platform, content_type, PlainText),
                    Available
                );
            }
            assert_eq!(write_availability(Other, content_type, Original), Available);
            assert_eq!(
                write_availability(Other, content_type, PlainText),
                Available
            );
        }
        for content_type in ["image/png", "image/tiff"] {
            assert_eq!(
                write_availability(Android, content_type, Original),
                Available
            );
            assert_eq!(write_availability(MacOs, content_type, Original), Available);
            assert_eq!(
                write_availability(Windows, content_type, Original),
                Available
            );
        }
        assert_eq!(
            write_availability(Windows, "image/bmp", Original),
            Available
        );
        assert_eq!(
            write_availability(MacOs, "image/bmp", Original),
            UnsupportedOnPlatform
        );
        assert_eq!(
            write_availability(Android, "image/bmp", Original),
            UnsupportedOnPlatform
        );
        assert_eq!(write_availability(Android, "file", Original), Available);
        assert_eq!(write_availability(MacOs, "file", Original), Available);
        assert_eq!(write_availability(Windows, "file", Original), Available);
        assert_eq!(
            write_availability(Other, "image/png", Original),
            UnsupportedOnPlatform
        );
        assert_eq!(
            write_availability(Other, "file", Original),
            UnsupportedOnPlatform
        );
        for platform in [Android, MacOs, Windows, Other] {
            for content_type in [
                "image/webp",
                "image/jpeg",
                "image/x-future",
                "application/x-future",
            ] {
                assert_eq!(
                    write_availability(platform, content_type, Original),
                    UnsupportedContentType,
                    "{content_type} on {platform:?}"
                );
            }
            for content_type in [
                "image/png",
                "image/tiff",
                "image/bmp",
                "image/webp",
                "file",
                "application/x-future",
            ] {
                assert_eq!(
                    write_availability(platform, content_type, PlainText),
                    UnsupportedContentType,
                    "plain text {content_type} on {platform:?}"
                );
            }
        }
    }

    #[test]
    fn clipboard_write_mode_accepts_only_the_two_wire_values() {
        assert_eq!(
            serde_json::from_str::<ClipboardWriteMode>("\"original\"").unwrap(),
            ClipboardWriteMode::Original
        );
        assert_eq!(
            serde_json::from_str::<ClipboardWriteMode>("\"plain_text\"").unwrap(),
            ClipboardWriteMode::PlainText
        );
        for invalid in ["\"binary\"", "\"plainText\"", "null", "42"] {
            assert!(serde_json::from_str::<ClipboardWriteMode>(invalid).is_err());
        }
    }

    #[test]
    fn clipboard_write_availability_has_stable_wire_values() {
        for (availability, expected) in [
            (ClipboardWriteAvailability::Available, "available"),
            (
                ClipboardWriteAvailability::UnsupportedContentType,
                "unsupported_content_type",
            ),
            (
                ClipboardWriteAvailability::UnsupportedOnPlatform,
                "unsupported_on_platform",
            ),
        ] {
            assert_eq!(
                serde_json::to_string(&availability).unwrap(),
                format!("\"{expected}\"")
            );
        }
    }

    /// `add_item` rejects blank content before spending a round trip, and the
    /// message it uses is one a user can act on.
    #[test]
    fn blank_content_is_rejected_with_an_actionable_message() {
        let err = BackendError::Invalid("There is nothing to add.");
        let shown = err.to_string();
        assert!(shown.ends_with('.'), "not a sentence: {shown}");
        assert!(!shown.contains('/'), "{shown}");
    }

    fn listed(id: &str, content: &str, content_type: &str) -> copypaste_ipc::Item {
        copypaste_ipc::Item {
            id: id.into(),
            content: content.into(),
            content_type: content_type.into(),
            created_at: 0,
            pinned: false,
            origin_device_id: "fake-device".into(),
            origin_device_name: None,
            source_app_bundle_id: None,
            source_app_name: None,
            too_large_to_sync: false,
            truncated: true,
        }
    }

    #[tokio::test]
    async fn a_bulk_copy_joins_whole_bodies_fetched_by_id() {
        let long = "y".repeat(copypaste_ipc::limits::LIST_PREVIEW_BYTES * 2);
        let backend = FakeBackend::failing().with_page(Page {
            items: vec![
                listed("a", &long, "text/plain"),
                listed("b", "second", "text/plain"),
            ],
            ..Page::default()
        });

        let (text, copied) = joined_text(&backend, &["a".to_string(), "b".to_string()])
            .await
            .unwrap();
        assert_eq!(text, format!("{long}\nsecond"));
        assert_eq!(copied, 2);
    }

    #[tokio::test]
    async fn a_bulk_copy_leaves_out_binary_items() {
        let backend = FakeBackend::failing().with_page(Page {
            items: vec![
                listed("a", "kept", "text/plain"),
                listed("b", "second", "text/plain"),
                listed("c", "[Image]", "image/png"),
            ],
            ..Page::default()
        });

        let (text, copied) = joined_text(
            &backend,
            &["a".to_string(), "b".to_string(), "c".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(text, "kept\nsecond");
        assert_eq!(copied, 2);
    }

    #[tokio::test]
    async fn a_bulk_copy_refuses_whole_rather_than_copying_a_fragment() {
        let backend = FakeBackend::failing().with_page(Page {
            items: vec![listed("a", "kept", "text/plain")],
            ..Page::default()
        });

        assert!(
            joined_text(&backend, &["a".to_string(), "gone".to_string()])
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_loaded_page_survives_an_independent_status_failure() {
        let backend = FakeBackend::failing().with_page(Page::default());

        // `FakeBackend::failing` rejects `status`. Listing must not depend on
        // it: the status query is separate and a daemon can restart between
        // two socket connections.
        let page = list_page(&backend, 50, None).await.unwrap();
        assert!(page.items().is_empty());
    }
}
