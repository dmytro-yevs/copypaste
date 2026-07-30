//! The bindings generator, as a binary in this crate.
//!
//! UniFFI's proc-macro mode reads the exported surface out of the compiled
//! library rather than out of a `.udl` file, so the generator has to be a
//! binary that links the same `uniffi` version this crate does. Building it
//! here is what guarantees that: a separately installed `uniffi-bindgen` can be
//! a different version, and a version skew between the generator and the
//! scaffolding is exactly the failure the checksums are there to catch — better
//! not to arrange it in the first place.
//!
//! Behind the `bindgen` feature, so an Android release build does not compile a
//! code generator into the shipped `.so`.
//!
//! ```text
//! cargo run -p copypaste-ffi --features bindgen --bin uniffi-bindgen -- \
//!     generate --library target/<triple>/release/libcopypaste_ffi.so \
//!     --language kotlin --out-dir apps/android/app/src/main/java
//! ```

fn main() {
    uniffi::uniffi_bindgen_main()
}
