//! Embeds the Windows version resource and application manifest.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/windows/executable.manifest");
    #[cfg(windows)]
    embed_windows_resource();
}

/// Version fields come from the Cargo package version.
#[cfg(windows)]
fn embed_windows_resource() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }
    winresource::WindowsResource::new()
        .set("CompanyName", "CopyPaste")
        .set("ProductName", "CopyPaste")
        .set("FileDescription", "CopyPaste daemon")
        .set("InternalName", "copypaste-daemon")
        .set("OriginalFilename", "copypaste-daemon.exe")
        .set_manifest(include_str!("../../packaging/windows/executable.manifest"))
        .compile_for("copypaste-daemon")
        .expect("compile the Windows version resource");
}
