// tests/slint_main_window.rs — headless Slint tests for the v0.4 MainWindow
// (redesigned RootWindow shell).
//
// Uses i-slint-backend-testing so no display is required.
//
// Regression coverage for the click-to-copy fixes:
//   1. `ClipItem.id` is a `string` (UUID), not an `int`. Before the fix the
//      id was typed `int`, so the daemon's UUID was mangled into a sentinel
//      integer and every copy_item / pin_item / delete_item call failed with
//      "invalid param: id must be a valid UUID". This test pins the id type
//      by round-tripping a real UUID through `history-items` + `detail-item`.
//   2. `item-clicked` / `item-copy` callbacks fire with the row index so the
//      Rust handler can resolve the clip and call `copy_item`.

use slint::{Model, ModelRc, VecModel};
use std::cell::Cell;
use std::rc::Rc;

slint::include_modules!();

fn init_backend() {
    i_slint_backend_testing::init_no_event_loop();
}

const SAMPLE_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

fn sample_items() -> Vec<ClipItem> {
    vec![
        ClipItem {
            id: SAMPLE_UUID.into(),
            preview: "first clip".into(),
            kind: "text".into(),
            wall_time: "2026-05-28 10:00:00".into(),
            source_device: "".into(),
            pinned: false,
            redacted: false,
        },
        ClipItem {
            id: "11111111-1111-1111-1111-111111111111".into(),
            preview: "second clip".into(),
            kind: "link".into(),
            wall_time: "2026-05-28 10:01:00".into(),
            source_device: "".into(),
            pinned: false,
            redacted: false,
        },
    ]
}

#[test]
fn clipitem_id_is_a_uuid_string_that_round_trips() {
    init_backend();
    let win = MainWindow::new().expect("MainWindow::new failed in headless mode");

    let model: Rc<VecModel<ClipItem>> = Rc::new(VecModel::from(sample_items()));
    win.set_history_items(ModelRc::from(model.clone()));

    // The id must survive verbatim — a regression to `int` would refuse to
    // compile this assignment / would truncate the UUID.
    let row0 = model.row_data(0).expect("row 0 exists");
    assert_eq!(
        row0.id.as_str(),
        SAMPLE_UUID,
        "ClipItem.id must round-trip the daemon UUID verbatim"
    );
}

#[test]
fn detail_item_carries_the_uuid_string() {
    init_backend();
    let win = MainWindow::new().expect("MainWindow::new");

    win.set_detail_item(ClipItem {
        id: SAMPLE_UUID.into(),
        preview: "detail clip".into(),
        kind: "text".into(),
        wall_time: "2026-05-28 10:00:00".into(),
        source_device: "".into(),
        pinned: false,
        redacted: false,
    });

    assert_eq!(
        win.get_detail_item().id.as_str(),
        SAMPLE_UUID,
        "detail-item.id must hold the UUID so detail-copy can call copy_item"
    );
}

#[test]
fn item_clicked_callback_delivers_index() {
    init_backend();
    let win = MainWindow::new().expect("MainWindow::new");

    let received = Rc::new(Cell::new(-1));
    win.on_item_clicked({
        let received = received.clone();
        move |idx| received.set(idx)
    });

    win.invoke_item_clicked(1);
    assert_eq!(
        received.get(),
        1,
        "item-clicked must deliver the clicked row index"
    );
}

#[test]
fn item_copy_callback_delivers_index() {
    init_backend();
    let win = MainWindow::new().expect("MainWindow::new");

    let received = Rc::new(Cell::new(-1));
    win.on_item_copy({
        let received = received.clone();
        move |idx| received.set(idx)
    });

    win.invoke_item_copy(0);
    assert_eq!(
        received.get(),
        0,
        "item-copy must deliver the row index so the handler can copy_item"
    );
}

#[test]
fn detail_copy_callback_fires() {
    init_backend();
    let win = MainWindow::new().expect("MainWindow::new");

    let fired = Rc::new(Cell::new(false));
    win.on_detail_copy({
        let fired = fired.clone();
        move || fired.set(true)
    });

    win.invoke_detail_copy();
    assert!(fired.get(), "detail-copy callback must fire when invoked");
}
