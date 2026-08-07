//! Main and Quick Paste window lifecycle.
//!
//! Native close and focus-loss events must use the same backend hide path as
//! commands. Otherwise close exits instead of leaving Quit to the tray (INV-36),
//! and a direct Quick Paste hide skips prior-app focus recovery, retains popup
//! data, or processes the trailing blur twice (INV-25, V-12, AT-44). Showing is
//! positioned before display and focused afterward because a hidden macOS
//! window ignores focus and moving an already-visible window causes a jump.
//!
//! Android compiles this shared module, but [`hide_window`] is a no-op there:
//! hiding the sole activity strands the user at the launcher.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

#[cfg(not(target_os = "android"))]
use tauri::{WebviewUrl, WebviewWindowBuilder};

#[cfg(target_os = "macos")]
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplicationActivationOptions, NSEvent, NSRunningApplication, NSWorkspace};

/// The full application surface. Named in `tauri.conf.json`.
pub const MAIN: &str = "main";

/// The compact, lazy-created Quick Paste surface.
pub const QUICK_PASTE: &str = "quick-paste";

/// The route loaded by the popup's separate WebView. Keeping it here makes a
/// full `App` mount in the popup impossible by construction: the builder never
/// points at the default route.
pub const QUICK_PASTE_ROUTE: &str = "index.html?surface=quick-paste";

/// The popup is security-sensitive from its first frame. A preference may only
/// relax this after the frontend has loaded, exactly like the main window.
pub const QUICK_PASTE_CONTENT_PROTECTED: bool = true;

/// The dynamic window's creation policy, kept as data so its security and
/// lifecycle properties can be tested without a native window server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickPasteWindowConfig {
    pub label: &'static str,
    pub route: &'static str,
    pub visible: bool,
    pub content_protected: bool,
}

pub const QUICK_PASTE_CONFIG: QuickPasteWindowConfig = QuickPasteWindowConfig {
    label: QUICK_PASTE,
    route: QUICK_PASTE_ROUTE,
    visible: false,
    content_protected: QUICK_PASTE_CONTENT_PROTECTED,
};

#[cfg(target_os = "macos")]
fn previous_application() -> &'static Mutex<Option<i32>> {
    static PREVIOUS_APPLICATION: OnceLock<Mutex<Option<i32>>> = OnceLock::new();
    PREVIOUS_APPLICATION.get_or_init(|| Mutex::new(None))
}

#[cfg(not(target_os = "android"))]
const QUICK_PASTE_WIDTH: f64 = 403.0;
#[cfg(not(target_os = "android"))]
const QUICK_PASTE_HEIGHT: f64 = 624.0;

/// Gap between the menu bar and the top of the popover, in physical pixels at
/// scale 1. Scaled by the target monitor's factor before use.
const GAP_PX: f64 = 6.0;

/// Keep this much of the screen edge clear, so the popover never sits flush
/// against the side of a display.
const EDGE_INSET_PX: f64 = 8.0;

/// The pointer should not land inside Quick Paste when it opens. Apart from
/// being visually surprising, opening underneath a held click turns that click
/// into an accidental row selection on some macOS versions.
#[cfg(target_os = "macos")]
const CURSOR_GAP_PX: f64 = 14.0;

pub fn main_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window(MAIN)
}

pub fn quick_paste_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window(QUICK_PASTE)
}

/// Show the full application surface. This is intentionally not the tray or
/// hotkey action; the popup's gear is the route here.
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    let _ = window.show();
    let _ = window.set_focus();
}

/// Open the main window on its settings route. The event is emitted only after
/// the window is visible, so its persistent webview cannot miss the request.
pub fn show_main_settings<R: Runtime>(app: &AppHandle<R>) {
    show_main(app);
    if let Some(window) = main_window(app) {
        let _ = window.eval(
            "window.__copypasteRequestedView = 'settings'; window.dispatchEvent(new Event('copypaste:open-settings'));",
        );
        let _ = window.emit("open-settings", ());
    }
}

/// Lazily create, position and focus the compact Quick Paste surface.
#[cfg(not(target_os = "android"))]
pub fn show_quick_paste<R: Runtime>(app: &AppHandle<R>) {
    let window = match quick_paste_window(app) {
        Some(window) => window,
        None => match create_quick_paste(app) {
            Ok(window) => window,
            Err(error) => {
                tracing::warn!(%error, "could not create the Quick Paste window");
                return;
            }
        },
    };
    if !window.is_visible().unwrap_or(false) {
        remember_frontmost_application();
    }
    // Position before showing it to avoid a visible jump. macOS has a pointer
    // location, so Quick Paste opens beside it; other desktop targets keep the
    // tray-anchor fallback.
    #[cfg(target_os = "macos")]
    if !position_under_cursor(&window) {
        position_under_tray(app, &window);
    }
    #[cfg(not(target_os = "macos"))]
    position_under_tray(app, &window);
    let _ = window.show();
    // Focus is what makes the search field live. Requested after `show`
    // because focusing a hidden window is a no-op on macOS.
    let _ = window.set_focus();
}

#[cfg(target_os = "android")]
pub fn show_quick_paste<R: Runtime>(_app: &AppHandle<R>) {}

#[cfg(not(target_os = "android"))]
fn create_quick_paste<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    WebviewWindowBuilder::new(
        app,
        QUICK_PASTE_CONFIG.label,
        WebviewUrl::App(QUICK_PASTE_CONFIG.route.into()),
    )
    .title("Quick Paste")
    .inner_size(QUICK_PASTE_WIDTH, QUICK_PASTE_HEIGHT)
    .resizable(false)
    .always_on_top(true)
    .decorations(false)
    .transparent(true)
    .shadow(true)
    .skip_taskbar(true)
    .visible(QUICK_PASTE_CONFIG.visible)
    .content_protected(QUICK_PASTE_CONFIG.content_protected)
    .build()
}

/// Hide whichever app surface issued the command.
///
/// A Quick Paste hide gives focus back to the application that was active
/// before it opened; we deliberately do not activate the main window on this
/// path.
pub fn hide_window<R: Runtime>(window: &WebviewWindow<R>) {
    if cfg!(target_os = "android") {
        return;
    }
    // Both a root blur and the native focus event can request this path. The
    // first one hides the popup; the second sees it already hidden and must
    // not reactivate the prior application a second time (V-12).
    if !window.is_visible().unwrap_or(true) {
        return;
    }

    if window.label() != QUICK_PASTE {
        let _ = window.hide();
        return;
    }

    hide_quick_paste_window(window, QuickPasteHideTarget::RestorePreviousApplication);
}

/// Hide a popup while moving into CopyPaste's main window. Restoring the
/// previous application here would immediately steal focus back from Settings.
pub fn hide_window_for_main<R: Runtime>(window: &WebviewWindow<R>) {
    if cfg!(target_os = "android") {
        return;
    }
    match main_transition_action(window.label()) {
        MainTransitionAction::HideQuickPaste(target) => hide_quick_paste_window(window, target),
        MainTransitionAction::HideWindow => {
            let _ = window.hide();
        }
    }
}

/// Hide Quick Paste through the same policy used by the frontend.
pub fn hide_quick_paste<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = quick_paste_window(app) {
        hide_window(&window);
    }
}

/// A hide produces a trailing blur on macOS. It must not trigger a second
/// focus hand-off after the popup is already gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusLossAction {
    Ignore,
    HideQuickPasteAndRestorePrior,
}

/// A close request must follow the same path as Escape, copy, and focus loss.
/// A direct `Window::hide` skips activation recovery and leaves the warm popup
/// data in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseRequestAction {
    HideQuickPaste(QuickPasteHideTarget),
    HideWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainTransitionAction {
    HideQuickPaste(QuickPasteHideTarget),
    HideWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuickPasteHideTarget {
    RestorePreviousApplication,
    MainWindow,
}

fn close_request_action(label: &str) -> CloseRequestAction {
    if label == QUICK_PASTE {
        CloseRequestAction::HideQuickPaste(QuickPasteHideTarget::RestorePreviousApplication)
    } else {
        CloseRequestAction::HideWindow
    }
}

fn main_transition_action(label: &str) -> MainTransitionAction {
    if label == QUICK_PASTE {
        MainTransitionAction::HideQuickPaste(QuickPasteHideTarget::MainWindow)
    } else {
        MainTransitionAction::HideWindow
    }
}

fn focus_loss_action(label: &str, visible: bool) -> FocusLossAction {
    if label == QUICK_PASTE && visible {
        FocusLossAction::HideQuickPasteAndRestorePrior
    } else {
        FocusLossAction::Ignore
    }
}

fn hide_after_window_event<R: Runtime>(window: &tauri::Window<R>, target: QuickPasteHideTarget) {
    if let Some(popup) = quick_paste_window(window.app_handle()) {
        if popup.is_visible().unwrap_or(false) {
            hide_quick_paste_window(&popup, target);
        }
    }
}

/// The popup retains its WebView to avoid reparsing the bundle. Its image cache
/// and item list must not survive a hide (AT-44), so ask the dedicated surface
/// to release them after native visibility changes.
fn free_quick_paste_memory<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.eval("window.__copypasteFreeMemory?.()");
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HideStep {
    Accessory,
    Hide,
    FreeMemory,
    Regular,
}

#[cfg(test)]
const NO_PRIOR_APPLICATION_HIDE: [HideStep; 4] = [
    HideStep::Accessory,
    HideStep::Hide,
    HideStep::FreeMemory,
    HideStep::Regular,
];

#[cfg(target_os = "macos")]
fn hide_quick_paste_window<R: Runtime>(window: &WebviewWindow<R>, target: QuickPasteHideTarget) {
    match target {
        QuickPasteHideTarget::MainWindow => {
            if window.hide().is_ok() {
                free_quick_paste_memory(window);
            }
        }
        QuickPasteHideTarget::RestorePreviousApplication => {
            if has_previous_application() {
                if window.hide().is_ok() {
                    free_quick_paste_memory(window);
                    restore_previous_application();
                }
                return;
            }

            // AT-39 / V-11: no prior app means CopyPaste itself was frontmost. Making
            // it an Accessory only while the popup disappears prevents macOS from
            // promoting the main window; Regular is restored immediately so Dock and
            // Cmd+Tab retain their normal main-app behaviour.
            let app = window.app_handle();
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            if window.hide().is_ok() {
                free_quick_paste_memory(window);
            }
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn hide_quick_paste_window<R: Runtime>(window: &WebviewWindow<R>, target: QuickPasteHideTarget) {
    if window.hide().is_ok() {
        free_quick_paste_memory(window);
        if target == QuickPasteHideTarget::RestorePreviousApplication {
            restore_previous_application();
        }
    }
}

/// What the hotkey and every tray entry point do.
pub fn toggle_quick_paste<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = quick_paste_window(app) else {
        show_quick_paste(app);
        return;
    };
    // `is_visible` failing is not a reason to do nothing: treating an
    // unanswerable window as hidden means the gesture still shows it, which is
    // the recoverable direction.
    if window.is_visible().unwrap_or(false) {
        hide_window(&window);
    } else {
        show_quick_paste(app);
    }
}

/// Place the window under the menu-bar item, when we can find out where that is.
///
/// Every failure here is silent and leaves the window where it was. A popover
/// in the wrong place is a cosmetic problem; a popover that refuses to appear
/// because a monitor query returned `None` is not.
#[cfg(not(target_os = "android"))]
fn position_under_tray<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let Ok(Some(rect)) = tray.rect() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };

    // The tray rect arrives in whatever unit the platform reported. Both arms
    // are converted to physical pixels against the monitor the icon is on,
    // because mixing the two is how a window lands on the wrong display when
    // the built-in screen and an external one have different scale factors.
    let scale = window.scale_factor().unwrap_or(1.0);
    let icon = to_physical(rect, scale);

    let Ok(Some(monitor)) = window.monitor_from_point(icon.centre_x, icon.bottom) else {
        return;
    };
    let scale = monitor.scale_factor();
    let position = anchor(icon, size, monitor.position(), monitor.size(), scale);
    let _ = window.set_position(position);
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn position_under_cursor<R: Runtime>(window: &WebviewWindow<R>) -> bool {
    let pointer = unsafe { NSEvent::mouseLocation() };
    let Ok(Some(primary)) = window.primary_monitor() else {
        return false;
    };
    let physical = cocoa_point_to_physical(
        pointer.x,
        pointer.y,
        primary.position(),
        primary.size(),
        primary.scale_factor(),
    );
    let monitor = window
        .monitor_from_point(f64::from(physical.x), f64::from(physical.y))
        .ok()
        .flatten()
        .unwrap_or(primary);
    let Ok(size) = window.outer_size() else {
        return false;
    };
    let position = cursor_anchor(
        physical,
        size,
        monitor.position(),
        monitor.size(),
        monitor.scale_factor(),
    );
    window.set_position(position).is_ok()
}

/// Place Quick Paste to the right and a little below the pointer. The same
/// bounds as the tray anchor apply, which makes this safe at every display edge
/// and on monitors with a negative origin.
#[cfg(target_os = "macos")]
fn cursor_anchor(
    pointer: PhysicalPosition<i32>,
    window: PhysicalSize<u32>,
    monitor_position: &PhysicalPosition<i32>,
    monitor_size: &PhysicalSize<u32>,
    scale: f64,
) -> PhysicalPosition<i32> {
    let width = f64::from(window.width);
    let height = f64::from(window.height);
    let inset = EDGE_INSET_PX * scale;
    let gap = CURSOR_GAP_PX * scale;

    let left = f64::from(monitor_position.x) + inset;
    let right = f64::from(monitor_position.x) + f64::from(monitor_size.width) - width - inset;
    let top = f64::from(monitor_position.y) + inset;
    let bottom = f64::from(monitor_position.y) + f64::from(monitor_size.height) - height - inset;

    let x = (f64::from(pointer.x) + gap).max(left).min(right.max(left));
    let y = (f64::from(pointer.y) + gap).max(top).min(bottom.max(top));
    PhysicalPosition::new(x.round() as i32, y.round() as i32)
}

#[cfg(target_os = "macos")]
fn cocoa_point_to_physical(
    x: f64,
    y: f64,
    monitor_position: &PhysicalPosition<i32>,
    monitor_size: &PhysicalSize<u32>,
    scale: f64,
) -> PhysicalPosition<i32> {
    PhysicalPosition::new(
        (x * scale).round() as i32,
        (f64::from(monitor_position.y) + f64::from(monitor_size.height) - y * scale).round() as i32,
    )
}

/// A tray icon's screen rectangle, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconRect {
    pub centre_x: f64,
    pub top: f64,
    pub bottom: f64,
}

#[cfg(not(target_os = "android"))]
fn to_physical(rect: tauri::Rect, scale: f64) -> IconRect {
    let position = match rect.position {
        tauri::Position::Physical(p) => PhysicalPosition::new(p.x as f64, p.y as f64),
        tauri::Position::Logical(p) => PhysicalPosition::new(p.x * scale, p.y * scale),
    };
    let size = match rect.size {
        tauri::Size::Physical(s) => PhysicalSize::new(s.width as f64, s.height as f64),
        tauri::Size::Logical(s) => PhysicalSize::new(s.width * scale, s.height * scale),
    };
    IconRect {
        centre_x: position.x + size.width / 2.0,
        top: position.y,
        bottom: position.y + size.height,
    }
}

/// Where the popover goes: centred on the icon, on whichever side of it there
/// is room for, clamped onto the monitor.
///
/// Split out from [`position_under_tray`] because it is the only part with a
/// right answer that can be checked — everything around it is platform state
/// this host cannot produce.
///
/// The clamp is what makes this correct on a second display. An icon near the
/// right edge of a wide screen would otherwise put half the window off it, and
/// on a monitor whose origin is negative (a display arranged to the left of the
/// primary) an unclamped calculation lands the window on the wrong screen
/// entirely.
pub fn anchor(
    icon: IconRect,
    window: PhysicalSize<u32>,
    monitor_position: &PhysicalPosition<i32>,
    monitor_size: &PhysicalSize<u32>,
    scale: f64,
) -> PhysicalPosition<i32> {
    let width = f64::from(window.width);
    let height = f64::from(window.height);
    let inset = EDGE_INSET_PX * scale;
    let gap = GAP_PX * scale;

    let left = f64::from(monitor_position.x) + inset;
    let right = f64::from(monitor_position.x) + f64::from(monitor_size.width) - width - inset;
    let top = f64::from(monitor_position.y) + inset;
    let bottom = f64::from(monitor_position.y) + f64::from(monitor_size.height) - height - inset;

    // `max` then `min`: on a monitor narrower than the window the window is
    // pinned to the left edge rather than pushed off the left one.
    let x = (icon.centre_x - width / 2.0).max(left).min(right.max(left));
    // Windows keeps the notification area on the taskbar, which sits at the
    // bottom of the screen by default. Hanging the popover *below* that icon
    // puts it behind the taskbar, and the clamp then pins it to the bottom
    // inset overlapping it. Decided from where the icon actually is, not from a
    // `cfg`: a taskbar the user moved to the top then behaves like a menu bar,
    // and a macOS menu bar is always in the upper half so nothing changes there.
    let y = if in_lower_half(&icon, monitor_position, monitor_size) {
        icon.top - gap - height
    } else {
        icon.bottom + gap
    };
    let y = y.max(top).min(bottom.max(top));

    PhysicalPosition::new(x.round() as i32, y.round() as i32)
}

fn in_lower_half(
    icon: &IconRect,
    monitor_position: &PhysicalPosition<i32>,
    monitor_size: &PhysicalSize<u32>,
) -> bool {
    let centre_y = (icon.top + icon.bottom) / 2.0;
    centre_y > f64::from(monitor_position.y) + f64::from(monitor_size.height) / 2.0
}

/// Window-event policy. Wired once in `crate::run`.
///
/// `Focused(false)` hides rather than closes so that the WebView keeps its
/// state — a popover that reloaded React on every dismissal would lose the
/// scroll position and the search text, and manifest 06's scroll-anchoring
/// requirements assume the list survives.
pub fn on_event<R: Runtime>(window: &tauri::Window<R>, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            match close_request_action(window.label()) {
                CloseRequestAction::HideQuickPaste(target) => {
                    hide_after_window_event(window, target)
                }
                CloseRequestAction::HideWindow => {
                    let _ = window.hide();
                }
            }
        }
        tauri::WindowEvent::Focused(false)
            if focus_loss_action(window.label(), window.is_visible().unwrap_or(false))
                == FocusLossAction::HideQuickPasteAndRestorePrior =>
        {
            hide_after_window_event(window, QuickPasteHideTarget::RestorePreviousApplication);
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn remember_frontmost_application() {
    // Captured before the popup receives focus. A PID, rather than an Objective-C
    // object, is retained so this cross-thread window state remains Send + Sync.
    let Ok(mut previous) = previous_application().lock() else {
        return;
    };
    *previous = None;
    let current = unsafe { NSRunningApplication::currentApplication().processIdentifier() };
    let frontmost = unsafe { NSWorkspace::sharedWorkspace().frontmostApplication() };
    let Some(frontmost) = frontmost else {
        return;
    };
    let pid = unsafe { frontmost.processIdentifier() };
    if pid != current {
        *previous = Some(pid);
    }
}

#[cfg(not(target_os = "macos"))]
fn remember_frontmost_application() {}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn take_previous_application() -> Option<i32> {
    previous_application()
        .lock()
        .ok()
        .and_then(|mut previous| previous.take())
}

#[cfg(target_os = "macos")]
fn has_previous_application() -> bool {
    previous_application()
        .lock()
        .is_ok_and(|previous| previous.is_some())
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn restore_previous_application() {
    let pid = take_previous_application();
    let Some(pid) = pid else {
        return;
    };
    let Some(application) =
        (unsafe { NSRunningApplication::runningApplicationWithProcessIdentifier(pid) })
    else {
        return;
    };
    let _ = unsafe { application.activateWithOptions(NSApplicationActivationOptions::empty()) };
}

#[cfg(not(target_os = "macos"))]
fn restore_previous_application() {}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1440×900 primary at the origin, and a 2560×1440 secondary arranged to
    /// its left — so the secondary's origin is negative, which is the case an
    /// unclamped calculation gets wrong.
    fn primary() -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
        (PhysicalPosition::new(0, 0), PhysicalSize::new(1440, 900))
    }

    fn secondary_on_the_left() -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
        (
            PhysicalPosition::new(-2560, 0),
            PhysicalSize::new(2560, 1440),
        )
    }

    const WINDOW: PhysicalSize<u32> = PhysicalSize {
        width: 420,
        height: 640,
    };

    #[test]
    fn quick_paste_has_its_own_window_label_and_route() {
        assert_ne!(MAIN, QUICK_PASTE);
        assert_eq!(QUICK_PASTE_CONFIG.label, "quick-paste");
        assert_eq!(QUICK_PASTE_CONFIG.route, "index.html?surface=quick-paste");
    }

    #[test]
    fn quick_paste_starts_hidden_and_protected() {
        const {
            assert!(!QUICK_PASTE_CONFIG.visible);
            assert!(QUICK_PASTE_CONFIG.content_protected);
        }
    }

    #[test]
    fn no_prior_application_hides_between_accessory_and_regular() {
        assert_eq!(
            NO_PRIOR_APPLICATION_HIDE,
            [
                HideStep::Accessory,
                HideStep::Hide,
                HideStep::FreeMemory,
                HideStep::Regular,
            ]
        );
    }

    #[test]
    fn settings_hides_the_popup_and_releases_its_cache() {
        assert_eq!(
            main_transition_action(QUICK_PASTE),
            MainTransitionAction::HideQuickPaste(QuickPasteHideTarget::MainWindow)
        );
        assert_eq!(
            main_transition_action(MAIN),
            MainTransitionAction::HideWindow
        );
    }

    #[test]
    fn close_requested_uses_the_popup_dismiss_lifecycle() {
        assert_eq!(
            close_request_action(QUICK_PASTE),
            CloseRequestAction::HideQuickPaste(QuickPasteHideTarget::RestorePreviousApplication)
        );
        assert_eq!(close_request_action(MAIN), CloseRequestAction::HideWindow);
        assert_eq!(
            NO_PRIOR_APPLICATION_HIDE,
            [
                HideStep::Accessory,
                HideStep::Hide,
                HideStep::FreeMemory,
                HideStep::Regular,
            ]
        );
    }

    #[test]
    fn blur_event_sequence_hides_only_popup_and_restores_once() {
        let actions = [
            focus_loss_action(QUICK_PASTE, true),
            // macOS delivers a trailing Focused(false) after `hide`.
            focus_loss_action(QUICK_PASTE, false),
            focus_loss_action(MAIN, true),
        ];

        assert_eq!(
            actions,
            [
                FocusLossAction::HideQuickPasteAndRestorePrior,
                FocusLossAction::Ignore,
                FocusLossAction::Ignore,
            ]
        );
        assert_eq!(
            actions
                .iter()
                .filter(|action| **action == FocusLossAction::HideQuickPasteAndRestorePrior)
                .count(),
            1
        );

        #[cfg(target_os = "macos")]
        {
            *previous_application().lock().unwrap() = Some(42);
            let restored = actions
                .iter()
                .filter(|action| **action == FocusLossAction::HideQuickPasteAndRestorePrior)
                .filter_map(|_| take_previous_application())
                .collect::<Vec<_>>();
            assert_eq!(restored, [42]);
            assert_eq!(take_previous_application(), None);
        }
    }

    /// A menu-bar icon: 24 physical pixels tall, at the top of the screen.
    fn menu_bar_icon(centre_x: f64) -> IconRect {
        IconRect {
            centre_x,
            top: 0.0,
            bottom: 24.0,
        }
    }

    /// A notification-area icon on a Windows taskbar at the bottom of the
    /// screen: 32 physical pixels tall, its bottom edge on the screen edge.
    fn taskbar_icon(centre_x: f64, monitor_bottom: f64) -> IconRect {
        IconRect {
            centre_x,
            top: monitor_bottom - 32.0,
            bottom: monitor_bottom,
        }
    }

    #[test]
    fn the_window_is_centred_under_the_icon_and_hangs_below_it() {
        let (pos, size) = primary();
        let at = anchor(menu_bar_icon(700.0), WINDOW, &pos, &size, 1.0);
        assert_eq!(at.x, 700 - 210);
        assert_eq!(at.y, 24 + GAP_PX as i32);
    }

    /// The Windows default. Below the icon is behind the taskbar and past the
    /// bottom of the display, so the popover has to open upwards.
    ///
    /// The notification area sits at the right end of the taskbar, so centring
    /// on the icon would put the window off the display: 1300 + 210 is past
    /// 1440. The right clamp, not the centring, is what places it here.
    #[test]
    fn an_icon_on_a_bottom_taskbar_opens_the_window_above_it() {
        let (pos, size) = primary();
        let at = anchor(taskbar_icon(1300.0, 900.0), WINDOW, &pos, &size, 1.0);
        assert_eq!(at.y, 900 - 32 - GAP_PX as i32 - WINDOW.height as i32);
        assert!(at.y + WINDOW.height as i32 <= 900 - 32, "{at:?}");
        assert_eq!(at.x, 1440 - WINDOW.width as i32 - EDGE_INSET_PX as i32);
    }

    /// The same taskbar moved to the top of the screen is a menu bar as far as
    /// this calculation is concerned, which is why the side is chosen from the
    /// icon's position rather than from the target platform.
    #[test]
    fn an_icon_in_the_upper_half_still_opens_downwards() {
        let (pos, size) = primary();
        let icon = IconRect {
            centre_x: 1300.0,
            top: 0.0,
            bottom: 32.0,
        };
        let at = anchor(icon, WINDOW, &pos, &size, 1.0);
        assert_eq!(at.y, 32 + GAP_PX as i32);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cocoa_cursor_is_flipped_into_tauri_screen_coordinates() {
        let (pos, size) = primary();
        let cursor = cocoa_point_to_physical(320.0, 180.0, &pos, &size, 2.0);
        assert_eq!(cursor, PhysicalPosition::new(640, 540));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn quick_paste_opens_to_the_right_of_the_cursor() {
        let (pos, size) = primary();
        let at = cursor_anchor(PhysicalPosition::new(420, 180), WINDOW, &pos, &size, 1.0);
        assert_eq!(
            at,
            PhysicalPosition::new(420 + CURSOR_GAP_PX as i32, 180 + CURSOR_GAP_PX as i32)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cursor_anchor_clamps_when_the_pointer_is_near_the_bottom_right_corner() {
        let (pos, size) = primary();
        let at = cursor_anchor(PhysicalPosition::new(1438, 898), WINDOW, &pos, &size, 1.0);
        assert_eq!(at.x, 1440 - WINDOW.width as i32 - EDGE_INSET_PX as i32);
        assert_eq!(at.y, 900 - WINDOW.height as i32 - EDGE_INSET_PX as i32);
    }

    /// The menu bar is at the right-hand end of the screen, which is where a
    /// menu-bar app's icon actually lives.
    #[test]
    fn an_icon_near_the_right_edge_does_not_push_the_window_off_screen() {
        let (pos, size) = primary();
        let at = anchor(menu_bar_icon(1430.0), WINDOW, &pos, &size, 1.0);
        assert!(
            at.x + 420 <= 1440,
            "the window ran off the right edge: x={}",
            at.x
        );
        assert_eq!(at.x, 1440 - 420 - EDGE_INSET_PX as i32);
    }

    /// The failure this clamp exists for: a display to the left of the primary
    /// has a negative origin, and an unclamped window lands on the wrong screen.
    #[test]
    fn a_monitor_with_a_negative_origin_keeps_the_window_on_that_monitor() {
        let (pos, size) = secondary_on_the_left();
        let at = anchor(menu_bar_icon(-2555.0), WINDOW, &pos, &size, 1.0);
        assert!(at.x >= -2560, "{at:?}");
        assert!(at.x + 420 <= 0, "{at:?}");
    }

    /// A Retina display reports scale 2, so the gap and the inset have to
    /// double with it or the popover sits visibly tight to the menu bar.
    #[test]
    fn the_gap_and_the_inset_scale_with_the_display() {
        let (pos, size) = primary();
        let icon = IconRect {
            centre_x: 700.0,
            top: 0.0,
            bottom: 48.0,
        };
        let at = anchor(icon, WINDOW, &pos, &size, 2.0);
        assert_eq!(at.y, 48 + (GAP_PX * 2.0) as i32);
    }

    /// Degenerate rather than impossible: a very small external display, or a
    /// window sized larger than the screen it is being placed on. Pinning to
    /// the top-left keeps the title area reachable; the alternative pushes the
    /// window's origin off the screen and makes it unmovable.
    #[test]
    fn a_window_larger_than_the_monitor_is_pinned_to_the_top_left() {
        let pos = PhysicalPosition::new(0, 0);
        let size = PhysicalSize::new(320, 240);
        let icon = IconRect {
            centre_x: 160.0,
            top: 0.0,
            bottom: 12.0,
        };
        let at = anchor(icon, WINDOW, &pos, &size, 1.0);
        assert_eq!(at.x, EDGE_INSET_PX as i32);
        assert_eq!(at.y, EDGE_INSET_PX as i32);
    }
}
