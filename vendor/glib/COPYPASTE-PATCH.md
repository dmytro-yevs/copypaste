Patched `glib` 0.18.5 for GHSA-wrw7-89jp-8q8g / RUSTSEC-2024-0429.

The GTK3/WebKitGTK 4.1 stack Tauri 2.11 still resolves cannot take upstream
`glib 0.20` without gtk4/webkit6. This tree is 0.18.5 with
`VariantStrIter::impl_get` passing `&mut p` to `g_variant_get_child`.

Keep the path patch until Tauri resolves `glib >= 0.20` from crates.io. Do not
renumber this crate to 0.20.0: Cargo will ignore the patch against `glib ^0.18`.
