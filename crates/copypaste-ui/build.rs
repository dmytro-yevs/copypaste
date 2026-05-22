// build.rs — compile Slint UI files for copypaste-ui
// All components exported via appui.slint so a single compile_with_config call is enough.

fn main() {
    slint_build::compile_with_config(
        "ui/appui.slint",
        slint_build::CompilerConfiguration::new().with_style("fluent-dark".into()),
    )
    .expect("Failed to compile Slint UI files");
}
