//! The AppKit compatibility seam for menu-item imagery.

#![allow(unsafe_code)]

use objc2_menu::rc::Retained;
use objc2_menu::runtime::AnyObject;
use objc2_menu::{msg_send, sel};
use tauri::{AppHandle, Runtime};

// NSMenuItemImageVisibility: automatic = 0, hidden = 1, visible = 2.
// The previous literal used `1`, which explicitly hid every image.
const IMAGE_VISIBILITY_VISIBLE: isize = 2;

/// macOS 27 hides menu-item images by default. `muda` still attaches each
/// image, so force AppKit to draw it after Tauri attaches the status menu.
pub(crate) fn force_menu_image_visibility<R: Runtime>(app: &AppHandle<R>) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let result = tray.with_inner_tray_icon(|inner| {
        let Some(status_item) = inner.ns_status_item() else {
            return;
        };
        let menu: Option<Retained<AnyObject>> = unsafe { msg_send![&*status_item, menu] };
        let Some(menu) = menu else {
            return;
        };
        force_images_in_menu(&menu);
    });
    if result.is_err() {
        tracing::debug!("could not set tray menu icon visibility");
    }
}

fn force_images_in_menu(menu: &AnyObject) {
    let items: Retained<AnyObject> = unsafe { msg_send![menu, itemArray] };
    let count: usize = unsafe { msg_send![&*items, count] };
    for index in 0..count {
        let item: Retained<AnyObject> = unsafe { msg_send![&*items, objectAtIndex: index] };
        let selector = sel!(setPreferredImageVisibility:);
        let responds: bool = unsafe { msg_send![&*item, respondsToSelector: selector] };
        if responds {
            // Older supported macOS versions do not implement this selector.
            let _: () =
                unsafe { msg_send![&*item, setPreferredImageVisibility: IMAGE_VISIBILITY_VISIBLE] };
        }
        let submenu: Option<Retained<AnyObject>> = unsafe { msg_send![&*item, submenu] };
        if let Some(submenu) = submenu {
            force_images_in_menu(&submenu);
        }
    }
}
