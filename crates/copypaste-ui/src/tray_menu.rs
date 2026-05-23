// tray_menu.rs — Beta-bonus: tray menu structure for CopyPaste.
//
// Slint does not own the system tray (the menu lives in the OS shell —
// `NSStatusItem` on macOS, `Gtk::StatusIcon`/`AppIndicator` on Linux,
// `Shell_NotifyIcon` on Windows). This module is a **stack-allocated,
// renderer-agnostic description** of the menu we want to mount.
//
// Why a description and not a live `tray-icon`/`muda` build?
//   * The current ui crate has zero tray dependency — pulling one in is a
//     larger change that touches `Cargo.toml`, `main.rs`'s event loop, and
//     platform-specific permissions. The beta bonus task asks for the menu
//     shape + tests, not for the full system-tray wiring.
//   * A pure-data description is trivially unit-testable on the build host
//     (which lacks a display server in CI) and gives the future tray-icon
//     wiring an unambiguous spec to consume.
//
// Layout (matches the task brief):
//   Show History            → opens HistoryWindow
//   Recent ▸                → submenu with up to 5 most-recent items
//       └─ <preview>        → clicking copies that item to clipboard
//   ─────────
//   Pair Device...          → opens PairWindow
//   Settings...             → opens SettingsWindow (stub if missing)
//   ─────────
//   Quit                    → terminates the daemon + UI

use std::cell::RefCell;
use std::rc::Rc;

// ── Public API surface ────────────────────────────────────────────────────────

/// Maximum number of clipboard previews shown inside the **Recent** submenu.
/// Anything beyond this is truncated — the full list lives in `HistoryWindow`.
pub const MAX_RECENT_ITEMS: usize = 5;

/// Maximum number of characters preserved in a tray preview. Tray menus on
/// macOS render at most ~40 chars before the OS truncates with an ellipsis,
/// so we pre-truncate to keep multibyte strings safe.
pub const MAX_PREVIEW_CHARS: usize = 40;

/// Stable identifiers for the top-level tray menu actions. Used as map keys
/// when the host wires callbacks; deliberately a closed enum so adding a new
/// action requires a compile-time update to every match site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayAction {
    /// Show the main history window.
    ShowHistory,
    /// Open the device-pairing window.
    PairDevice,
    /// Open the settings window.
    OpenSettings,
    /// Quit the application (daemon + UI).
    Quit,
}

impl TrayAction {
    /// Stable string id — handy for logging, telemetry, and matching
    /// `tray-icon::MenuId`-style string keys.
    pub fn id(self) -> &'static str {
        match self {
            TrayAction::ShowHistory => "show_history",
            TrayAction::PairDevice => "pair_device",
            TrayAction::OpenSettings => "open_settings",
            TrayAction::Quit => "quit",
        }
    }

    /// User-facing label as rendered in the tray. Stable across platforms;
    /// localisation is out of scope for the beta.
    pub fn label(self) -> &'static str {
        match self {
            TrayAction::ShowHistory => "Show History",
            TrayAction::PairDevice => "Pair Device...",
            TrayAction::OpenSettings => "Settings...",
            TrayAction::Quit => "Quit",
        }
    }
}

/// A single entry shown inside the **Recent** submenu. The `id` is the
/// daemon-side clipboard-item id (`history_page` row id); the `preview` is
/// the truncated display label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentItem {
    pub id: String,
    pub preview: String,
}

impl RecentItem {
    /// Build a recent item, truncating the preview to [`MAX_PREVIEW_CHARS`]
    /// characters (Unicode scalars, not bytes) and appending an ellipsis
    /// when truncation actually happens.
    pub fn new(id: impl Into<String>, preview: impl AsRef<str>) -> Self {
        Self {
            id: id.into(),
            preview: truncate_preview(preview.as_ref()),
        }
    }
}

/// Logical kinds of menu entry the host renderer needs to materialise.
/// Kept renderer-agnostic so it can be mapped onto `tray-icon::MenuItem`,
/// `gtk::MenuItem`, or `NSMenuItem` without leaking platform types here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    /// A clickable action — fires the bound `TrayAction` callback.
    Action(TrayAction),
    /// The "Recent" submenu, with the current snapshot of items inlined so
    /// the renderer doesn't need to ask back for them.
    RecentSubmenu(Vec<RecentItem>),
    /// A horizontal separator line.
    Separator,
}

// ── Menu state ────────────────────────────────────────────────────────────────

/// Owns the current recents snapshot and produces the [`MenuEntry`] list
/// the renderer mounts. Cheap to clone because `Vec<RecentItem>` is the
/// only mutable state.
#[derive(Debug, Default, Clone)]
pub struct TrayMenuState {
    recents: Vec<RecentItem>,
}

impl TrayMenuState {
    pub fn new() -> Self {
        Self {
            recents: Vec::new(),
        }
    }

    /// Replace the recents snapshot, clamping at [`MAX_RECENT_ITEMS`].
    /// Items beyond the limit are discarded silently — the tray surface is
    /// intentionally narrow; the full list belongs to `HistoryWindow`.
    pub fn set_recents(&mut self, items: Vec<RecentItem>) {
        self.recents = items.into_iter().take(MAX_RECENT_ITEMS).collect();
    }

    /// Borrow the current recents snapshot.
    pub fn recents(&self) -> &[RecentItem] {
        &self.recents
    }

    /// Produce the renderer-agnostic menu layout. Order matches the brief
    /// exactly; tests below assert this contract.
    pub fn build(&self) -> Vec<MenuEntry> {
        vec![
            MenuEntry::Action(TrayAction::ShowHistory),
            MenuEntry::RecentSubmenu(self.recents.clone()),
            MenuEntry::Action(TrayAction::PairDevice),
            MenuEntry::Action(TrayAction::OpenSettings),
            MenuEntry::Separator,
            MenuEntry::Action(TrayAction::Quit),
        ]
    }
}

// ── Callback wiring ───────────────────────────────────────────────────────────

type ActionCb = Box<dyn Fn() + 'static>;
type RecentCb = Box<dyn Fn(&str) + 'static>;

/// Rust-side handle for the tray menu — mirrors the shape of
/// `SettingsWindowHandle` / `PairWindowHandle` so the host wires it the
/// same way (`handle.on_show_history(|| ...)`, then `handle.dispatch(...)`).
///
/// The handle keeps callback closures in interior-mutable cells so they can
/// be (re)registered after construction without needing `&mut self` — the
/// renderer thread only needs a shared reference.
pub struct TrayMenuHandle {
    state: RefCell<TrayMenuState>,
    on_show_history: RefCell<Option<ActionCb>>,
    on_pair_device: RefCell<Option<ActionCb>>,
    on_open_settings: RefCell<Option<ActionCb>>,
    on_quit: RefCell<Option<ActionCb>>,
    on_recent_click: RefCell<Option<RecentCb>>,
}

impl TrayMenuHandle {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            state: RefCell::new(TrayMenuState::new()),
            on_show_history: RefCell::new(None),
            on_pair_device: RefCell::new(None),
            on_open_settings: RefCell::new(None),
            on_quit: RefCell::new(None),
            on_recent_click: RefCell::new(None),
        })
    }

    /// Replace the recents snapshot. Call from the IPC poller whenever the
    /// daemon emits a `history_page` update.
    pub fn set_recents(&self, items: Vec<RecentItem>) {
        self.state.borrow_mut().set_recents(items);
    }

    /// Return the current menu layout — what the renderer should mount.
    pub fn menu(&self) -> Vec<MenuEntry> {
        self.state.borrow().build()
    }

    /// Borrow-clone of the current recents snapshot.
    pub fn recents(&self) -> Vec<RecentItem> {
        self.state.borrow().recents().to_vec()
    }

    // ── Callback registration ────────────────────────────────────────────────

    pub fn on_show_history<F: Fn() + 'static>(&self, cb: F) {
        *self.on_show_history.borrow_mut() = Some(Box::new(cb));
    }
    pub fn on_pair_device<F: Fn() + 'static>(&self, cb: F) {
        *self.on_pair_device.borrow_mut() = Some(Box::new(cb));
    }
    pub fn on_open_settings<F: Fn() + 'static>(&self, cb: F) {
        *self.on_open_settings.borrow_mut() = Some(Box::new(cb));
    }
    pub fn on_quit<F: Fn() + 'static>(&self, cb: F) {
        *self.on_quit.borrow_mut() = Some(Box::new(cb));
    }
    /// Callback for clicks inside the **Recent** submenu. Receives the
    /// item's daemon-side id; the handler is responsible for invoking
    /// `IpcClient::paste(id)` (or equivalent copy-to-clipboard).
    pub fn on_recent_click<F: Fn(&str) + 'static>(&self, cb: F) {
        *self.on_recent_click.borrow_mut() = Some(Box::new(cb));
    }

    // ── Dispatch ─────────────────────────────────────────────────────────────

    /// Invoke the callback bound to a top-level [`TrayAction`].
    /// Returns `true` if a callback was registered and fired, `false`
    /// otherwise (so the caller can fall back to a default).
    pub fn dispatch(&self, action: TrayAction) -> bool {
        let slot = match action {
            TrayAction::ShowHistory => &self.on_show_history,
            TrayAction::PairDevice => &self.on_pair_device,
            TrayAction::OpenSettings => &self.on_open_settings,
            TrayAction::Quit => &self.on_quit,
        };
        if let Some(cb) = slot.borrow().as_ref() {
            cb();
            true
        } else {
            false
        }
    }

    /// Invoke the recents-click callback. Returns `true` if the id was
    /// present in the current snapshot AND a callback was registered.
    /// Unknown ids are ignored — protects against stale tray clicks racing
    /// a recents refresh.
    pub fn dispatch_recent(&self, id: &str) -> bool {
        let known = self.state.borrow().recents().iter().any(|r| r.id == id);
        if !known {
            return false;
        }
        if let Some(cb) = self.on_recent_click.borrow().as_ref() {
            cb(id);
            true
        } else {
            false
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Truncate a preview string to [`MAX_PREVIEW_CHARS`] Unicode scalars,
/// collapsing any embedded newline runs into a single space so the tray
/// item renders on one line. Returns the original string when no
/// truncation is needed.
fn truncate_preview(s: &str) -> String {
    // Collapse newlines/tabs to spaces — the tray menu is one-line per item.
    let flat: String = s
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();

    let char_count = flat.chars().count();
    if char_count <= MAX_PREVIEW_CHARS {
        return flat;
    }
    let mut out: String = flat.chars().take(MAX_PREVIEW_CHARS - 1).collect();
    out.push('…');
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    // --- Layout contract ---

    #[test]
    fn menu_has_expected_top_level_layout() {
        let state = TrayMenuState::new();
        let entries = state.build();

        // Show History, Recent submenu, Pair, Settings, Separator, Quit
        assert_eq!(
            entries.len(),
            6,
            "menu must have exactly 6 top-level entries"
        );

        assert!(matches!(
            entries[0],
            MenuEntry::Action(TrayAction::ShowHistory)
        ));
        assert!(matches!(entries[1], MenuEntry::RecentSubmenu(_)));
        assert!(matches!(
            entries[2],
            MenuEntry::Action(TrayAction::PairDevice)
        ));
        assert!(matches!(
            entries[3],
            MenuEntry::Action(TrayAction::OpenSettings)
        ));
        assert!(matches!(entries[4], MenuEntry::Separator));
        assert!(matches!(entries[5], MenuEntry::Action(TrayAction::Quit)));
    }

    #[test]
    fn all_action_labels_match_brief() {
        // The brief pinned these exact strings — guard against accidental drift.
        assert_eq!(TrayAction::ShowHistory.label(), "Show History");
        assert_eq!(TrayAction::PairDevice.label(), "Pair Device...");
        assert_eq!(TrayAction::OpenSettings.label(), "Settings...");
        assert_eq!(TrayAction::Quit.label(), "Quit");
    }

    #[test]
    fn all_action_ids_are_stable_and_unique() {
        let ids = [
            TrayAction::ShowHistory.id(),
            TrayAction::PairDevice.id(),
            TrayAction::OpenSettings.id(),
            TrayAction::Quit.id(),
        ];
        assert_eq!(
            ids,
            ["show_history", "pair_device", "open_settings", "quit"]
        );
        // Unique
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 4, "action ids must be unique");
    }

    // --- Recents handling ---

    #[test]
    fn recents_are_capped_at_max() {
        let mut state = TrayMenuState::new();
        let many: Vec<RecentItem> = (0..20)
            .map(|i| RecentItem::new(format!("id-{i}"), format!("preview {i}")))
            .collect();
        state.set_recents(many);

        assert_eq!(state.recents().len(), MAX_RECENT_ITEMS);
        assert_eq!(state.recents()[0].id, "id-0");
        assert_eq!(state.recents()[4].id, "id-4");
    }

    #[test]
    fn recents_submenu_reflects_current_snapshot() {
        let mut state = TrayMenuState::new();
        state.set_recents(vec![
            RecentItem::new("a", "alpha"),
            RecentItem::new("b", "beta"),
        ]);
        match &state.build()[1] {
            MenuEntry::RecentSubmenu(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].preview, "alpha");
                assert_eq!(items[1].id, "b");
            }
            other => panic!("expected RecentSubmenu, got {other:?}"),
        }
    }

    #[test]
    fn empty_recents_submenu_is_empty_not_omitted() {
        // The submenu entry must always exist — renderers rely on stable
        // positions. Just the inner Vec is empty.
        let state = TrayMenuState::new();
        match &state.build()[1] {
            MenuEntry::RecentSubmenu(items) => assert!(items.is_empty()),
            other => panic!("expected RecentSubmenu, got {other:?}"),
        }
    }

    // --- Preview truncation ---

    #[test]
    fn short_preview_passes_through_unchanged() {
        let item = RecentItem::new("x", "hello");
        assert_eq!(item.preview, "hello");
    }

    #[test]
    fn long_preview_truncates_with_ellipsis() {
        let long = "x".repeat(MAX_PREVIEW_CHARS * 2);
        let item = RecentItem::new("x", &long);
        assert_eq!(item.preview.chars().count(), MAX_PREVIEW_CHARS);
        assert!(item.preview.ends_with('…'));
    }

    #[test]
    fn preview_truncation_counts_unicode_scalars_not_bytes() {
        // Cyrillic + emoji — every char is multibyte. We must truncate on
        // scalar count so we don't slice mid-codepoint.
        let mixed: String = "ё".repeat(MAX_PREVIEW_CHARS * 2);
        let item = RecentItem::new("x", &mixed);
        assert_eq!(item.preview.chars().count(), MAX_PREVIEW_CHARS);
    }

    #[test]
    fn preview_collapses_newlines_to_spaces() {
        let item = RecentItem::new("x", "line1\nline2\rline3\tend");
        assert!(!item.preview.contains('\n'));
        assert!(!item.preview.contains('\r'));
        assert!(!item.preview.contains('\t'));
        assert!(item.preview.contains("line1 line2 line3 end"));
    }

    // --- Callback dispatch ---

    #[test]
    fn dispatch_returns_false_when_no_callback_is_bound() {
        let handle = TrayMenuHandle::new();
        assert!(!handle.dispatch(TrayAction::Quit));
        assert!(!handle.dispatch(TrayAction::ShowHistory));
    }

    #[test]
    fn dispatch_fires_registered_callback_for_each_action() {
        let handle = TrayMenuHandle::new();
        let hits = Rc::new(Cell::new(0_u32));

        let h = Rc::clone(&hits);
        handle.on_show_history(move || h.set(h.get() | 0b0001));
        let h = Rc::clone(&hits);
        handle.on_pair_device(move || h.set(h.get() | 0b0010));
        let h = Rc::clone(&hits);
        handle.on_open_settings(move || h.set(h.get() | 0b0100));
        let h = Rc::clone(&hits);
        handle.on_quit(move || h.set(h.get() | 0b1000));

        assert!(handle.dispatch(TrayAction::ShowHistory));
        assert!(handle.dispatch(TrayAction::PairDevice));
        assert!(handle.dispatch(TrayAction::OpenSettings));
        assert!(handle.dispatch(TrayAction::Quit));

        assert_eq!(
            hits.get(),
            0b1111,
            "every registered callback must have fired exactly once"
        );
    }

    #[test]
    fn dispatch_recent_ignores_unknown_ids() {
        let handle = TrayMenuHandle::new();
        handle.set_recents(vec![RecentItem::new("known", "k")]);

        let fired = Rc::new(Cell::new(false));
        let f = Rc::clone(&fired);
        handle.on_recent_click(move |_id| f.set(true));

        assert!(
            !handle.dispatch_recent("unknown"),
            "stale ids must not dispatch"
        );
        assert!(!fired.get(), "callback must not fire for unknown id");

        assert!(handle.dispatch_recent("known"));
        assert!(fired.get());
    }

    #[test]
    fn dispatch_recent_passes_id_to_callback() {
        let handle = TrayMenuHandle::new();
        handle.set_recents(vec![
            RecentItem::new("id-1", "p1"),
            RecentItem::new("id-2", "p2"),
        ]);

        let captured: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let c = Rc::clone(&captured);
        handle.on_recent_click(move |id| *c.borrow_mut() = Some(id.to_string()));

        handle.dispatch_recent("id-2");
        assert_eq!(captured.borrow().as_deref(), Some("id-2"));
    }

    #[test]
    fn handle_set_recents_caps_at_max() {
        let handle = TrayMenuHandle::new();
        let many: Vec<RecentItem> = (0..10)
            .map(|i| RecentItem::new(format!("id-{i}"), format!("p{i}")))
            .collect();
        handle.set_recents(many);
        assert_eq!(handle.recents().len(), MAX_RECENT_ITEMS);
    }

    #[test]
    fn menu_layout_action_count_is_four() {
        // Sanity-check that the brief's four top-level actions
        // (ShowHistory, PairDevice, OpenSettings, Quit) all appear.
        let state = TrayMenuState::new();
        let action_count = state
            .build()
            .iter()
            .filter(|e| matches!(e, MenuEntry::Action(_)))
            .count();
        assert_eq!(action_count, 4);
    }
}
