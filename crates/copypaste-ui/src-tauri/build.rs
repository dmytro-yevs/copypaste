use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    link_atomic_builtins();
    tauri_build::build()
}

// Vendored OpenSSL's threads_pthread.c uses 64-bit atomics, which are the one
// builtin family Rust's compiler_builtins does not carry and which only 32-bit
// x86 needs out of line — the other three ABIs resolve them inline. rustc links
// Android with -nodefaultlibs, so clang adds no compiler-rt, and the NDK's
// libatomic.a is a comment saying the __atomic_* APIs moved into
// libclang_rt.builtins. The -latomic the target spec already passes therefore
// answers nothing, and :app:rustBuildX86Release took v2.0.0-alpha.5 down on
// undefined __atomic_load_8. Name that archive ourselves (ADR-0007).
fn link_atomic_builtins() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android")
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86")
    {
        return;
    }

    let cc = cc::Build::new()
        .try_get_compiler()
        .expect("no C compiler for i686-linux-android; the NDK's clang belongs in TARGET_CC");

    // With cc's own target flags: bare `clang -print-libgcc-file-name` answers
    // for the host, and a host archive on this link line is silently useless.
    let out = Command::new(cc.path())
        .args(cc.args())
        .arg("-print-libgcc-file-name")
        .output()
        .unwrap_or_else(|e| panic!("could not run {}: {e}", cc.path().display()));
    let archive = String::from_utf8(out.stdout).expect("compiler-rt path is not UTF-8");
    let archive = archive.trim();

    assert!(
        out.status.success() && archive.contains("i686") && Path::new(archive).is_file(),
        "{} -print-libgcc-file-name did not name the i686 compiler-rt archive: {archive:?}",
        cc.path().display()
    );
    println!("cargo:rustc-link-arg={archive}");
}
