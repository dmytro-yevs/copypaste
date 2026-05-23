# Unsafe Code Review — v0.3.0 — 2026-05-23

Read-only audit of every `unsafe {}` block in the workspace, per THREAT-MODEL
OI-8.  Focus: the newly-landed raw-FFI module `keychain/acl.rs` (commit
`47cfcee`) plus the pre-existing macOS NSPasteboard and test-env-mutation
sites.

## Summary

| Metric | Value |
|---|---|
| Total `unsafe` blocks | 25 (excluding 2 `#![forbid(unsafe_code)]` declarations) |
| Files containing unsafe | 5 |
| New since v0.2 (commit 47cfcee) | 15 (all in `keychain/acl.rs`) |
| HIGH severity findings | 1 |
| MEDIUM severity findings | 3 |
| LOW severity findings | 4 |
| SAFETY-comment coverage | 7 / 25 (28%) |

All `unsafe` is FFI-shaped (Security.framework, Core Foundation, AppKit
`NSPasteboard`, `std::env::set_var`). No raw-pointer arithmetic, no
`transmute`, no `unsafe impl Send/Sync`, no inline asm.

## Findings

### HIGH — must fix in v0.3

#### F-1: `keychain/acl.rs:231–236` — `CFArray::from_copyable` uses NULL callbacks; comment claiming the array retains its entries is **incorrect**

**Code:**
```rust
// crates/copypaste-daemon/src/keychain/acl.rs:227–236
// Convert the Vec<*mut OpaqueSecTrustedApplication> into a CFArray<CFType>.
// CFArray takes a copy of the raw pointers (CFArrayCreate retains
// each entry under the kCFTypeArrayCallBacks default), so we still
// own the originals and must release them ourselves below.
let cf_array: CFArray<CFTypeRef> = CFArray::from_copyable(
    &trusted_apps
        .iter()
        .map(|p| *p as CFTypeRef)
        .collect::<Vec<_>>(),
);
```

**Issue.** `CFArray::from_copyable` (core-foundation 0.10.1, `src/array.rs:66`)
calls `CFArrayCreate(... , ptr::null())` — it passes a **null** callback
pointer.  With a null callback table CF does **not** retain/release its
entries; the array stores raw bit-patterns.  The comment in `acl.rs:228–230`
asserts the opposite (`"CFArrayCreate retains each entry under the
kCFTypeArrayCallBacks default"`).  The correct sibling helper is
`CFArray::from_CFTypes` (same file, line 82) which explicitly passes
`&kCFTypeArrayCallBacks` and DOES retain.

**Impact.** Today this happens to be safe because:
1. `SecAccessCreate` is synchronous and consumes the trust list during the
   call.
2. The `Vec<SecTrustedApplicationRef>` is still alive (we release it after
   the IIFE returns).
3. So the pointers passed into the CFArray remain valid for the duration
   of `SecAccessCreate`.

But the comment is factually wrong and the next maintainer is likely to
trust it.  If anyone reorders the release loop above the `SecAccessCreate`
call, or extracts `cf_array` outside this scope, **every entry becomes a
dangling pointer immediately** — silent UAF inside Security.framework.

**Fix.** Switch to a wrapper that retains, or document the actual ordering
invariant prominently:

```rust
// Option A — retaining array (preferred):
use core_foundation::base::TCFType;
let elems: Vec<CFTypeRef> = trusted_apps.iter().map(|p| *p as CFTypeRef).collect();
let cf_array_ref = unsafe {
    core_foundation_sys::array::CFArrayCreate(
        std::ptr::null(),
        elems.as_ptr(),
        elems.len() as isize,
        &core_foundation_sys::array::kCFTypeArrayCallBacks,
    )
};
let cf_array: CFArray<CFTypeRef> =
    unsafe { CFArray::wrap_under_create_rule(cf_array_ref) };

// Option B — keep from_copyable but FIX the comment to:
//
// SAFETY: from_copyable creates a CFArray with NULL callbacks — the
// array does NOT retain its entries. This is safe ONLY because
// `trusted_apps` outlives `SecAccessCreate` (we release entries after
// the IIFE returns). DO NOT release any `trusted_apps[i]` before
// SecAccessCreate completes.
```

Option A is strictly safer and removes the load-bearing comment.

---

### MEDIUM — should fix before 1.0

#### M-1: `keychain/acl.rs:212` — `CString::new(path.as_os_str().as_encoded_bytes())` can return `Err(NulError)` silently lost as `AclPathEncoding`

**Code:**
```rust
let c_path = CString::new(path.as_os_str().as_encoded_bytes())
    .map_err(|_| KeychainError::AclPathEncoding)?;
```

**Issue.** `OsStr::as_encoded_bytes` returns the raw WTF-8 bytes including
any embedded NUL.  On Unix a filesystem path technically cannot contain a
NUL, but the path here originates from `std::env::current_exe()` and
`parent.join(name)` — both of which take user-controllable bundle layouts.
A path with an embedded NUL would map to a generic `AclPathEncoding` error,
losing the specific cause and (more importantly) silently dropping that
binary from the trust list when this is called from `trusted_binary_paths()`
filtering.

**Impact.** Misleading error; low practical risk on macOS.

**Fix.** Either keep the generic error but `tracing::warn!(?path,
"keychain ACL skipped path: contains NUL byte")`, or surface a specific
`KeychainError::PathContainsNul(PathBuf)` variant.

#### M-2: `keychain/acl.rs:296–313` — release of `out_item` even when `SecKeychainItemCreateFromContent` failed and the contract for `out_item` on failure is undocumented

**Code:**
```rust
let mut out_item: SecKeychainItemRef = ptr::null_mut();
let status = unsafe {
    SecKeychainItemCreateFromContent(/* ... */ access, &mut out_item)
};
unsafe { CFRelease(access as CFTypeRef) };
if !out_item.is_null() {
    unsafe { CFRelease(out_item as CFTypeRef) };
}
if status != ERR_SEC_SUCCESS { return Err(...); }
```

**Issue.** Apple's documentation does not guarantee that `out_item` is
NULL on failure.  Some legacy Security.framework calls leave the out
parameter unmodified — i.e. NULL because we pre-zeroed it — but a few
return a partially-constructed item alongside a non-zero OSStatus.  Today
the code happens to do the right thing (release if non-null) but the
SAFETY rationale is missing and the next person may swap the `is_null`
check for `if status == SUCCESS`, which would leak on the partial-failure
path.

Also missing: there is no `// SAFETY:` comment explaining that `access`
must outlive the call (it does, because we hold the +1 ref until the
explicit `CFRelease(access)` below the call returns).

**Impact.** Possible leak if someone refactors the null-check based on
the OSStatus.

**Fix.** Add `// SAFETY:` block explaining:
- `&attr_list` is a valid stack borrow with `count = attrs.len()`;
- `secret.as_ptr()` is valid for `secret.len()` bytes (compile-time
  guaranteed by `&[u8; 32]`);
- `access` is a +1 ref owned by us and released exactly once below;
- `out_item` is unconditionally released if non-null because Apple does
  not promise it is NULL on failure.

#### M-3: `keychain/acl.rs:347, 356, 358, 376–383` — `current_acl_app_digests`: per-ACL inner loop continues past `SecACLCopyContents` failure but still leaks `description` if `app_list` was populated on a failed call

**Code:**
```rust
let status = unsafe {
    SecACLCopyContents(*acl, &mut app_list, &mut description, &mut prompt_selector)
};
if status != ERR_SEC_SUCCESS {
    continue;             // <-- skips description/app_list release
}
if !description.is_null() {
    unsafe { CFRelease(description as CFTypeRef) };
}
if app_list.is_null() {
    continue;
}
```

**Issue.** If `SecACLCopyContents` fails AFTER having populated `description`
or `app_list` (rare but documented for Apple's "copy" APIs that allocate
output before validating subsequent steps), `continue` skips the release
of those buffers — a leak per failed ACL entry.  Over the lifetime of the
daemon this is bounded (one call at startup) but the pattern propagates.

**Impact.** Small one-shot leak of CFType refs on a failure path.

**Fix.** Move the null-checked releases up so they run on the error path
too:

```rust
let status = unsafe { SecACLCopyContents(*acl, &mut app_list, &mut description, &mut prompt_selector) };
// Release outputs unconditionally on any outcome.
let _release_guards = (
    !description.is_null()
        .then(|| unsafe { CFRelease(description as CFTypeRef) }),
    // app_list handed off below via wrap_under_create_rule if status == SUCCESS
);
if status != ERR_SEC_SUCCESS { continue; }
```

Or restructure with a small RAII guard struct.

---

### LOW — cleanup / hygiene

#### L-1: `keychain/acl.rs:215, 240, 260, 296, 310, 312, 347, 356, 358, 370, 376, 383, 388, 391, 398` — every `unsafe` block in this file lacks a `// SAFETY:` comment

Every other `unsafe` in the workspace (paths.rs, daemon.rs tests) has a
`SAFETY:` comment.  The new module has zero — 15 unsafe blocks, 0
comments.  This is a clippy-`undocumented_unsafe_blocks` lint violation
waiting to happen and a real maintainability hazard given the subtle
CFRetain/CFRelease balancing the module relies on.

**Fix.** Add a one-line `// SAFETY:` to each `unsafe` describing the
invariant that makes it sound.  Roughly:

| Line | SAFETY rationale |
|---|---|
| 215 | `c_path` is a valid C string owned by this scope; `&mut app` is a stack-borrowed out-param. |
| 240 | All three CFRefs (`cf_descriptor`, `cf_array`, `access_ref`) live for the call. |
| 260 | `app` is a +1 ref from `SecTrustedApplicationCreateFromPath`; releasing exactly once. |
| 296 | `attr_list`, `secret.as_ptr()`, `access` all valid for call duration; `out_item` is a pre-zeroed out-param. |
| 310 | `access` is the +1 ref returned by `SecAccessCreate`; releasing exactly once. |
| 312 | `out_item` is non-null and is the +1 ref returned by `SecKeychainItemCreateFromContent`. |
| 347 | `item_ref` borrowed from `legacy_find`'s returned item, alive for this call. |
| 356 | `access_ref` is the +1 ref from `SecKeychainItemCopyAccess`. |
| 358 | Releasing the +1 ref from line 347 exactly once. |
| 370 | `acl_array` is a +1 ref; `wrap_under_create_rule` takes ownership. |
| 376 | All four out-params are stack-borrowed; safe to pass. |
| 383 | `description` is a non-null +1 ref from `SecACLCopyContents`. |
| 388 | `app_list` is a +1 ref; `wrap_under_create_rule` takes ownership. |
| 391 | `*app` is a borrowed CFTypeRef from the wrapped CFArray, alive for the call. |
| 398 | `data` is a non-null +1 ref from `SecTrustedApplicationCopyData`. |

#### L-2: `keychain/acl.rs` — module-level `#![allow(non_snake_case, non_upper_case_globals, deprecated)]` is broad

The `deprecated` allow is necessary (Sec Keychain* is deprecated since
10.10) but it also silences any *future* deprecation we'd want to know
about (e.g. if Apple removes a symbol in 16.x).  Consider narrowing:

```rust
#[allow(deprecated)]
mod legacy_ffi { ... extern "C" { ... } }
```

so we only silence deprecations on the FFI surface itself, not on
business logic that happens to live in the same file.

#### L-3: `clipboard.rs:132` — single 60-line `unsafe { ... }` block is wider than needed

The block spans all of pasteboard probing.  Half the calls
(`NSString::from_str`, `.to_string()`) are safe.  Narrowing the `unsafe`
scope to just the actual FFI calls (`NSPasteboard::generalPasteboard()`,
`changeCount()`, `stringForType`, `dataForType`) would surface accidental
non-FFI mutations more loudly.  Functionally fine today; hygiene only.

#### L-4: `paths.rs` and `daemon.rs` test-env-mutation `unsafe` blocks are correctly documented but rely on a per-test mutex (`ENV_LOCK`) which is bypassed by two tests (`paths_returns_error_when_home_unset`, `device_id_persists_across_restart`)

Both tests note this in the SAFETY comment ("env mutation is process-global
and racy with parallel tests") and mitigate by restoring values, but they
do not take `ENV_LOCK`.  Under `cargo test -j N` they can race with the
ENV_LOCK-protected tests in the same module, causing intermittent failures
in CI.  Not a soundness issue — Rust's `set_var` is just `unsafe` because
the libc semantics are global — but a correctness flag worth tracking.

**Fix.** Either take `ENV_LOCK` in those tests too, or annotate them with
`#[serial]` (would add `serial_test` dep).

---

## Aggregated stats

| Crate / File | unsafe blocks | FFI | Send/Sync | Raw ptr deref | std::env |
|---|---|---|---|---|---|
| copypaste-daemon/src/keychain/acl.rs | 15 | 15 (Security.framework + CoreFoundation) | 0 | 0 | 0 |
| copypaste-daemon/src/paths.rs | 5 | 0 | 0 | 0 | 5 (test env) |
| copypaste-daemon/src/daemon.rs | 2 | 0 | 0 | 0 | 2 (test env) |
| copypaste-daemon/src/ipc.rs | 2 | 2 (NSPasteboard set) | 0 | 0 | 0 |
| copypaste-daemon/src/clipboard.rs | 1 | 1 (NSPasteboard read) | 0 | 0 | 0 |
| copypaste-config/src/lib.rs | 0 (`#![forbid(unsafe_code)]`) | — | — | — | — |
| copypaste-telemetry/src/lib.rs | 0 (`#![forbid(unsafe_code)]`) | — | — | — | — |
| copypaste-core | 0 | — | — | — | — |
| copypaste-android | 0 | — | — | — | — |
| copypaste-cli | 0 | — | — | — | — |
| copypaste-relay | 0 | — | — | — | — |
| copypaste-ui | 0 | — | — | — | — |
| **TOTAL** | **25** | **18** | **0** | **0** | **7** |

Two additional crates (`copypaste-config`, `copypaste-telemetry`) actively
enforce `#![forbid(unsafe_code)]` — consider adding the same lint to
`copypaste-core`, `copypaste-cli`, `copypaste-relay`, `copypaste-ui`,
`copypaste-android` to prevent silent unsafe creep in crates that should
never need it.

## SAFETY comment coverage

| File | Documented | Undocumented |
|---|---|---|
| keychain/acl.rs | 0 / 15 | 15 |
| paths.rs | 5 / 5 | 0 |
| daemon.rs | 2 / 2 | 0 |
| ipc.rs | 0 / 2 | 2 |
| clipboard.rs | 0 / 1 | 1 |
| **TOTAL** | **7 / 25** (28%) | **18** |

Undocumented `unsafe` blocks (file:line):
- `keychain/acl.rs`: 215, 240, 260, 296, 310, 312, 347, 356, 358, 370, 376, 383, 388, 391, 398
- `ipc.rs`: 1052, 1065
- `clipboard.rs`: 132

## Recommendations for v0.3 follow-up

1. **(HIGH) Fix F-1 before v0.3.0-rc1.** Either switch to a retaining
   CFArray constructor or update the comment to explicitly document the
   load-bearing ordering invariant.  Add a regression test that calls
   `build_access` and immediately drops `trusted_apps` (would tsan/asan
   under `cargo test -Z sanitizer=address` catch the dangling pointer if
   refactored wrong).
2. **(LOW) Add `// SAFETY:` comments to every unsafe block** in
   `keychain/acl.rs`, `ipc.rs`, `clipboard.rs` — at minimum copy the
   table from L-1 above.  Then enable
   `#![deny(clippy::undocumented_unsafe_blocks)]` on the daemon crate.
3. **(MEDIUM) Tighten error mapping in M-1** so paths with embedded NULs
   log a specific diagnostic instead of generic `AclPathEncoding`.
4. **(MEDIUM) Apply M-2 SAFETY block to `store_with_acl`** documenting
   that `out_item` is unconditionally released because Apple does not
   guarantee NULL on failure.

## Recommendations for v0.4+

1. **Cover the FFI module with `cargo miri test`** for the host-target
   paths that don't actually call Security.framework (the path-encoding
   helpers, the trust-list assembly), and run macOS-only integration
   tests under AddressSanitizer in CI on macos-14 runners.
2. **Consider extracting `keychain/acl.rs` into a separate crate**
   `copypaste-keychain-ffi` with `#![deny(unsafe_op_in_unsafe_fn,
   clippy::undocumented_unsafe_blocks)]` — gives the auditor a single
   blast-radius file and makes the rest of the daemon `#![forbid(unsafe_code)]`-able.
3. **Add `#![forbid(unsafe_code)]` to** `copypaste-core`, `copypaste-cli`,
   `copypaste-relay`, `copypaste-ui` to prevent silent unsafe regression.
4. **L-2 narrowing:** wrap the legacy-Security FFI in an inner module so
   `#[allow(deprecated)]` does not bleed into business logic.
5. **L-3 hygiene pass on `clipboard.rs` and `ipc.rs`** to narrow `unsafe`
   scopes to actual FFI calls only.
