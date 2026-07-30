// Hide the console window on Windows release builds. Windows is out of scope
// (CLAUDE.md rule 5); this is the one line Tauri's template carries for it and
// costs nothing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    copypaste_ui_lib::run()
}
