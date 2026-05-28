// build.rs — compile the *existing* copypaste-ui Slint files (read-only) so the
// snapshot harness can instantiate `MainWindow` and friends without copying or
// editing any `.slint` source.
//
// We point slint-build at `../copypaste-ui/ui/appui.slint` (the same root entry
// the real UI crate compiles). All `import` paths inside appui.slint are
// relative, so they resolve against that directory automatically. We use the
// identical `fluent-dark` style so the rendered pixels match the shipped app.

use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR is always set by cargo when running build scripts"),
    );

    // crates/copypaste-ui-snapshot -> crates/copypaste-ui/ui/appui.slint
    let ui_dir = manifest_dir.join("..").join("copypaste-ui").join("ui");
    let entry = ui_dir.join("appui.slint");

    assert!(
        entry.exists(),
        "expected copypaste-ui entry point at {}; the snapshot harness reads the UI crate's \
         .slint files in place and must not be moved away from it",
        entry.display()
    );

    slint_build::compile_with_config(
        &entry,
        slint_build::CompilerConfiguration::new().with_style("fluent-dark".into()),
    )
    .expect("failed to compile copypaste-ui Slint files for snapshotting");
}
