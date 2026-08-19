# ADR-0021: Remediate the glib VariantStrIter unsoundness

Status: accepted (in-tree patch; scanner still matches 0.18.5)
Owner: `@dmytro-yevs`

## Decision

Ship the upstream `VariantStrIter::impl_get` mutability fix (`&mut p` into
`g_variant_get_child`) by patching `glib` to `vendor/glib`. The crate version
stays **0.18.5** so `[patch.crates-io]` still satisfies Tauri 2.11's `glib ^0.18`.

A crates.io bump to 0.20 needs gtk4/webkit6, which Tauri 2.11 does not take.
cargo-audit therefore no longer reports RUSTSEC-2024-0429 against the path
crate. `scripts/check_rustsec_policy.py` still fails if `vendor/glib` loses
`&mut p`. Dependabot may lag until it treats the lockfile path package as
outside the crates.io advisory.

## Constraint

Tauri 2.11 still resolves webkit2gtk 2.0.2 → `glib ^0.18`. Keep the path patch
until that graph takes `glib >= 0.20` from crates.io, then delete `vendor/glib`
and the exception.
