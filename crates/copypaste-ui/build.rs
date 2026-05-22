// build.rs — compile all Slint UI files via the appui.slint root entry point.

fn main() {
    slint_build::compile_with_config(
        "ui/appui.slint",
        slint_build::CompilerConfiguration::new().with_style("fluent-dark".into()),
    )
    .expect("Failed to compile Slint UI files");
}
