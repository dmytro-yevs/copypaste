//! macOS UNUserNotificationCenter integration — post banners after copies,
//! query permission, and build rich title/body pairs from clipboard item data.

use copypaste_ipc::{METHOD_GET_CONFIG, METHOD_HISTORY_PAGE};

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Show a rich macOS notification banner after a successful copy.
///
/// Posts via `UNUserNotificationCenter` from inside the `CopyPaste.app`
/// bundle so the notification automatically shows the app icon.  This
/// replaces the old `osascript display notification` path which ran from
/// a process with no bundle identity and therefore showed a generic
/// `Script Editor` icon with no app icon and no rich preview.
///
/// Parameters (set by the frontend):
/// - `title`: short type label, e.g. "Text Copied", "Image Copied",
///   "File Copied".
/// - `body`: item preview — first ~160 chars of text (newlines preserved,
///   truncated with `…`), the filename for files, or "Image" for images.
///
/// Authorization: on macOS 10.14+ the first call triggers the system
/// permission prompt.  If the user denies it or the request fails, the
/// error is silently swallowed — this is purely cosmetic feedback.
///
/// The command is cross-platform safe: on non-macOS it is a no-op.
#[tauri::command]
pub(crate) fn show_copy_notification(title: String, body: String) {
    #[cfg(target_os = "macos")]
    {
        post_un_notification(title, body);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (title, body);
    }
}

/// Check whether macOS system notification permission is granted for this app.
///
/// Queries `UNUserNotificationCenter.current().notificationSettings.authorizationStatus`
/// and returns `true` when the status is `.authorized`.  This is the correct
/// signal for whether `show_copy_notification` will actually deliver a banner
/// from `UNUserNotificationCenter` — it is entirely separate from the browser
/// `Notification.permission` API (which is meaningless inside a Tauri WKWebView).
///
/// On non-macOS platforms always returns `true` (no per-app notification
/// permission model exists there).
///
/// CopyPaste-44rq.28: this command is called by `notificationPermission.ts` via
/// `invoke("check_notification_permission")`.
#[tauri::command]
pub(crate) fn check_notification_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        query_notification_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Synchronously query the macOS UNUserNotificationCenter authorization status.
///
/// Uses a Rust mpsc channel as a one-shot semaphore: the ObjC completion block
/// fires on an unspecified background queue and sends the result; this function
/// blocks on `recv_timeout` so the Tauri command handler (which is driven by a
/// Tokio blocking thread via `spawn_blocking` when async, or the main thread
/// when sync) does not busy-spin.
///
/// Returns `true` when `authorizationStatus == .authorized`, `false` for
/// `.denied`, `.notDetermined`, `.provisional`, `.ephemeral`, or any error.
#[cfg(target_os = "macos")]
pub(crate) fn query_notification_permission() -> bool {
    use block2::RcBlock;
    use core::ptr::NonNull;
    use objc2_user_notifications::{
        UNAuthorizationStatus, UNNotificationSettings, UNUserNotificationCenter,
    };

    // Channel used as a one-shot semaphore: the completion block sends a bool;
    // we block here on recv_timeout.
    let (tx, rx) = std::sync::mpsc::channel::<bool>();

    // SAFETY: UNUserNotificationCenter::currentNotificationCenter() is
    // documented as safe to call from any thread. The completion block captures
    // `tx` and is called exactly once on an internal GCD queue; `mpsc::Sender`
    // is Send so the capture is sound.
    unsafe {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        // Block signature: dyn Fn(NonNull<UNNotificationSettings>)
        // — matches the generated binding in UNUserNotificationCenter.rs.
        let block = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            // SAFETY: `settings` is a non-null pointer to a valid
            // UNNotificationSettings object owned by the ObjC runtime for
            // the duration of this block invocation.
            let status = settings.as_ref().authorizationStatus();
            let granted = status == UNAuthorizationStatus::Authorized;
            // Receiver may have dropped if we timed out, which is fine.
            let _ = tx.send(granted);
        });
        center.getNotificationSettingsWithCompletionHandler(&block);
    }

    // Wait up to 2 s for the ObjC callback. On a healthy system it fires
    // within milliseconds; the timeout guards against pathological hangs.
    match rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(v) => v,
        Err(_) => {
            // Timed out or channel closed — fall back to false (conservative:
            // assume denied) so the "Notifications disabled" warning appears.
            tracing::debug!(
                "check_notification_permission: timed out waiting for \
                 UNNotificationSettings; assuming denied"
            );
            false
        }
    }
}

/// Post a `UNUserNotificationCenter` banner from the app bundle.
///
/// Called on a background thread (spawned by the Tauri command handler or
/// the background-capture poller) so the main run-loop is never blocked.
/// Any failure is logged at DEBUG level and silently swallowed.
#[cfg(target_os = "macos")]
pub(crate) fn post_un_notification(title: String, body: String) {
    std::thread::spawn(move || {
        use block2::RcBlock;
        // objc2_foundation_v3 is the aliased objc2-foundation 0.3.x crate that
        // matches the types used by objc2-user-notifications 0.3.x (which
        // depends on objc2 0.6.x).  The rest of the crate uses the 0.2.x
        // bindings required by objc2-app-kit 0.2.x; both coexist in the graph.
        use objc2_foundation_v3::{NSError, NSString};
        // Bool from objc2 0.6.x — must match the version used by
        // objc2-user-notifications 0.3.x for the IntoBlock impl to unify.
        use objc2_user_notifications::{
            UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
            UNUserNotificationCenter,
        };
        use objc2_v6::runtime::Bool;

        // SAFETY: All ObjC calls below are on the same thread; objc2 0.6.x
        // enforces Send/Sync on retained objects so cross-thread use is safe.
        // `UNUserNotificationCenter::currentNotificationCenter()` is documented
        // as safe to call from any thread.
        unsafe {
            let center = UNUserNotificationCenter::currentNotificationCenter();

            // Request `.alert` authorization — shows a system prompt on first
            // call; subsequent calls return the cached decision immediately.
            let auth_opts = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Badge;
            // The closure parameter types must match the DynBlock signature
            // generated by objc2-user-notifications 0.3.x:
            //   dyn Fn(Bool, *mut NSError)
            // where Bool and NSError come from objc2 0.6.x / objc2-foundation 0.3.x.
            let auth_block = RcBlock::new(move |granted: Bool, err: *mut NSError| {
                if !granted.as_bool() {
                    if err.is_null() {
                        tracing::debug!("post_un_notification: notification permission denied");
                    } else {
                        // SAFETY: err is non-null and owned by the ObjC runtime.
                        let msg = (*err).localizedDescription();
                        tracing::debug!("post_un_notification: auth error: {}", msg);
                    }
                }
            });
            center.requestAuthorizationWithOptions_completionHandler(auth_opts, &auth_block);

            // Build content — title + body.
            // NSString::from_str returns Retained<NSString>; bind before
            // passing so we can deref to &NSString as required by setTitle/setBody.
            let content = UNMutableNotificationContent::new();
            let ns_title = NSString::from_str(&title);
            let ns_body = NSString::from_str(&body);
            content.setTitle(&ns_title);
            content.setBody(&ns_body);

            // Unique identifier per notification so each fires independently.
            let req_id = NSString::from_str(&format!(
                "com.copypaste.copy.{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));

            // trigger = None → deliver immediately.
            // req_id is Retained<NSString>; deref to &NSString for the call.
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &req_id, &content, None,
            );

            // Completion block: dyn Fn(*mut NSError) — same NSError from 0.3.x.
            let done_block = RcBlock::new(move |err: *mut NSError| {
                if !err.is_null() {
                    // SAFETY: err is non-null and owned by the ObjC runtime.
                    let msg = (*err).localizedDescription();
                    tracing::debug!("post_un_notification: add request failed: {}", msg);
                }
            });
            center.addNotificationRequest_withCompletionHandler(&request, Some(&done_block));
        }
    });
}

// ---------------------------------------------------------------------------
// Notification content helpers
// ---------------------------------------------------------------------------

/// Build a rich notification `(title, body)` pair from a `copy_item` IPC
/// reply.
///
/// The daemon now returns `content_type` and `preview` in the `copy_item`
/// response.  This helper maps them to the human-readable strings shown in
/// the macOS notification banner.
///
/// - Text → `("Text Copied", first ~160 chars truncated with …)`
/// - Image → `("Image Copied", "Image")`
/// - File → `("File Copied", filename extracted from "[file: <name>]")`
/// - Unknown → `("Copied", preview-as-body or "Copied")`
pub(crate) fn notification_title_body_from_reply(reply: &crate::ipc::IpcReply) -> (String, String) {
    let content_type = reply
        .data
        .as_ref()
        .and_then(|d| d["content_type"].as_str())
        .unwrap_or("");
    let preview = reply
        .data
        .as_ref()
        .and_then(|d| d["preview"].as_str())
        .unwrap_or("");

    notification_title_body(content_type, preview)
}

/// Build a rich notification `(title, body)` pair from `content_type` and
/// `preview` strings (the shape returned by `history_page` and `copy_item`).
pub(crate) fn notification_title_body(content_type: &str, preview: &str) -> (String, String) {
    match content_type {
        "text" => {
            let body = build_text_preview_body(preview);
            ("Text Copied".to_owned(), body)
        }
        ct if ct == "image" || ct.starts_with("image/") => {
            ("Image Copied".to_owned(), "Image".to_owned())
        }
        "file" => {
            // preview arrives as "[file: <filename>]" from history_page.
            // Strip the wrapper to show just the filename in the banner.
            let body = if let Some(inner) = preview
                .strip_prefix("[file: ")
                .and_then(|s| s.strip_suffix(']'))
            {
                inner.to_owned()
            } else if !preview.is_empty() {
                preview.to_owned()
            } else {
                "File".to_owned()
            };
            ("File Copied".to_owned(), body)
        }
        _ => {
            // Fallback: unknown or empty content_type.
            let body = build_text_preview_body(preview);
            (
                "Copied".to_owned(),
                if body.is_empty() {
                    "Copied".to_owned()
                } else {
                    body
                },
            )
        }
    }
}

/// Truncate a text `preview` to ~160 chars at a word boundary and append `…`
/// if truncated.  Preserves newlines so multi-line text reads naturally in the
/// notification banner (macOS renders them as line breaks).
pub(crate) fn build_text_preview_body(preview: &str) -> String {
    const MAX_CHARS: usize = 160;
    // Compare CHARS, not bytes: `preview.len()` is the UTF-8 byte length, which
    // for multibyte text (Cyrillic, emoji) overstates the visible length and
    // would truncate far earlier than the intended 160-char budget.
    if preview.chars().count() <= MAX_CHARS {
        return preview.to_owned();
    }
    // Truncate at MAX_CHARS chars (not bytes), preferring a word boundary.
    let truncated: String = preview.chars().take(MAX_CHARS).collect();
    // Walk back to the last whitespace for a clean cut.
    let cut = truncated
        .rfind(|c: char| c.is_whitespace())
        .unwrap_or(MAX_CHARS);
    let chopped = truncated[..cut].trim_end();
    if chopped.is_empty() {
        format!("{}…", truncated.trim_end())
    } else {
        format!("{chopped}…")
    }
}

/// Poll for the most recent clipboard item and fire a notification if a new
/// background capture appeared since the last check.
///
/// `last_seen` is updated in-place so subsequent calls only notify once per
/// item.  Respects the daemon's `notify_on_copy` setting.
pub(crate) fn check_and_notify_new_capture(last_seen: &mut i64) {
    // Fetch the single most-recent item from history_page (limit=1).
    let reply = match crate::ipc::call(
        METHOD_HISTORY_PAGE,
        serde_json::json!({ "limit": 1, "offset": 0 }),
    ) {
        Ok(r) if r.ok => r,
        _ => return,
    };

    let item = match reply
        .data
        .as_ref()
        .and_then(|d| d["items"].as_array())
        .and_then(|a| a.first())
    {
        Some(i) => i.clone(),
        None => return,
    };

    let wall_time = match item["wall_time"].as_i64() {
        Some(t) => t,
        None => return,
    };

    if wall_time <= *last_seen {
        return; // no new item
    }

    *last_seen = wall_time;

    // Check notify_on_copy setting before firing.
    let notify_enabled = crate::ipc::call(METHOD_GET_CONFIG, serde_json::json!({}))
        .ok()
        .and_then(|r| r.data)
        .and_then(|d| d["notify_on_copy"].as_bool())
        .unwrap_or(false);

    if !notify_enabled {
        return;
    }

    let content_type = item["content_type"].as_str().unwrap_or("").to_owned();
    let preview = item["preview"].as_str().unwrap_or("").to_owned();
    let (title, body) = notification_title_body(&content_type, &preview);

    #[cfg(target_os = "macos")]
    post_un_notification(title, body);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (title, body);
    }
}
