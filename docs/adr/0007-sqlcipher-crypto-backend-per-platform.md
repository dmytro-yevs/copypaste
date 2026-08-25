# ADR-0007 — SQLCipher's crypto backend, per platform

**Status:** accepted · 2026-07-30
**Scope:** which crypto library SQLCipher is compiled against on each shipped
platform, and what the build must guarantee about how it is linked.

## Context

Storage is SQLCipher, via `rusqlite`'s `bundled-sqlcipher` feature, which
compiles `sqlite3.c` from source. SQLCipher is not self-contained: it delegates
AES and PBKDF2 to a provider chosen at compile time. `libsqlite3-sys` wires up
exactly two — Apple's CommonCrypto and OpenSSL — and picks between them in
`build.rs` with no way to state a preference directly:

| Condition | Result |
|---|---|
| `bundled-sqlcipher-vendored-openssl` enabled | OpenSSL, built from source by `openssl-src`, statically linked |
| `OPENSSL_DIR`, or `OPENSSL_LIB_DIR` + `OPENSSL_INCLUDE_DIR`, set | `-lcrypto` **dynamically**, against the build machine's OpenSSL |
| otherwise, host and target both Apple | `-DSQLCIPHER_CRYPTO_CC`, CommonCrypto, linking only Security and CoreFoundation |
| otherwise | `-lcrypto` dynamically, against the system OpenSSL |

Two things follow, and they are the whole of this decision.

**Android has no fourth branch to fall into.** It is not Apple, so it takes the
last row and emits `#include <openssl/crypto.h>`. The NDK sysroot ships no
OpenSSL headers or libraries at all, so the APK build fails while compiling
`sqlite3.c`. There is nothing to point `OPENSSL_DIR` at.

**macOS's branch is selected by an absence.** With no `OPENSSL_*` variable set
it uses CommonCrypto, which is part of the OS and needs nothing bundled. If a
runner image ever starts exporting `OPENSSL_DIR`, the same source and the same
command produce a binary dynamically linked against that machine's libcrypto —
on a GitHub runner, Homebrew's, under `/opt/homebrew`. That DMG mounts and
launches on the runner and fails with a dyld error on every user's Mac, and
nothing in the pipeline would catch it, because the pipeline has no install
test.

## Decision

**Vendor OpenSSL for the Android targets only. Leave every other target on the
backend it already resolves to, and verify the macOS result rather than trusting
it.**

Three parts:

1. Android enables `bundled-sqlcipher-vendored-openssl` through a
   target-specific dependency, so the feature exists only where it is needed:

   ```toml
   [target.'cfg(target_os = "android")'.dependencies]
   rusqlite = { workspace = true, features = ["bundled-sqlcipher-vendored-openssl"] }
   ```

   This must live in a crate manifest — `[workspace.dependencies]` has no
   per-target form, and the root manifest is virtual and cannot carry
   dependencies at all.

2. `deny.toml` scopes `openssl-sys` with `wrappers = ["libsqlite3-sys"]` rather
   than dropping the ban. Reintroducing OpenSSL via `native-tls`, or via a
   `reqwest` that lost its rustls features, still fails.

3. `scripts/release/check-macos-linkage.sh` runs in the macOS release job and
   rejects any Mach-O whose load commands or `LC_RPATH` entries name a path
   outside `/usr/lib`, `/System/Library`, or the bundle.

Resolver v2 is what makes part 1 work rather than silently vendoring
everywhere. Verified with `cargo tree -e features`: the host target resolves no
`openssl-sys` at all, `aarch64-linux-android` resolves `openssl-sys` with
`vendored` and `openssl-src`. `Cargo.lock` lists `openssl-sys` regardless —
lockfiles are target-agnostic — which is why part 2 scopes the ban instead of
relying on the crate's absence.

## The cost, stated

AGENTS.md rule 1 asks for the cost written down rather than assumed either way.
This is not a second crypto stack arriving — SQLCipher already required
OpenSSL, and the alternative was not "no OpenSSL" but "no Android build" — but
vendoring one is still a decision that buys something and spends something.

- **Build time.** `openssl-src` compiles OpenSSL 3.x from source, once per ABI,
  for four ABIs. Cold, this is the dominant cost of the Android job. Warm, it is
  free: `Swatinem/rust-cache` already saves `target/` on release runs.
- **Toolchain.** `openssl-src` shells out to `perl` and `make`. Both are present
  on `ubuntu-24.04`. `macos-14` never runs this path.
- **APK size.** A static libcrypto per ABI in a universal APK.
- **Audit surface.** OpenSSL's advisory feed now applies to us. `cargo-deny` and
  `cargo-audit` already run on a schedule, so this is covered rather than new
  work, but it is a real addition.
- **Not paid on macOS.** CommonCrypto costs no build time, no binary size, and
  no advisory surface, which is the main reason this is scoped rather than
  global.

## Consequences

The two shipped platforms use different SQLCipher providers. This is safe
because a SQLCipher database file never crosses a device boundary: the
encrypted store is local, and peer and cloud sync move application-layer
ciphertext, not database pages. Both providers implement the same SQLCipher 4
defaults, so the difference is not observable in the file format either.

`openssl-src` maps all four Android ABIs onto generic OpenSSL configurations
(`linux-aarch64`, `linux-armv4`, `linux-elf`, `linux-x86_64`) and takes its
compiler, archiver and ranlib from `cc`, which it hands to `./Configure` to be
baked into the Makefile. It does not read `ANDROID_NDK_ROOT`.

**The ranlib has to be wired in; the rest arrives.** The Tauri Android build
(`cargo-mobile2`) exports `TARGET_CC`, `TARGET_CXX` and `TARGET_AR` with
absolute NDK paths, and no `RANLIB` at all. Given none, `cc` probes `PATH` for
`llvm-ranlib` and otherwise falls back to `<triple>-ranlib` — a GCC-era wrapper
the NDK deleted in r23 — so the fourth release dry run compiled OpenSSL for
`aarch64-linux-android` and then died in `install_dev` on
`aarch64-linux-android-ranlib: not found`.

`scripts/release/android-ndk-env.sh` exports `AR_<triple>` and
`RANLIB_<triple>` (underscored — the highest-priority spelling `cc` reads that
a shell can export) for all four ABIs, pointing at the NDK's `llvm-ar` and
`llvm-ranlib`. `AR` is set alongside `RANLIB` rather than left to
`cargo-mobile2`, whose contract has already proved partial. The alternative —
a `PATH` directory of `<triple>-ar` symlinks — is the widely copied recipe and
works whatever reads what, but it rots invisibly: nothing would ever report a
shim that had stopped being used. The script instead asserts the two tools are
executable before the build starts, and `check.sh` runs it against a fake NDK
and asserts the exact variable names, because a misspelled one is not an error,
only a variable nobody reads.

**So do the 32-bit x86 atomics.** `threads_pthread.c`'s RCU uses 64-bit
atomics, which only `i686-linux-android` resolves out of line. Nothing on the
link line answers those calls: rustc passes `-nodefaultlibs`, so clang
contributes no compiler-rt; Rust's `compiler_builtins` carries no `__atomic_*`;
and the NDK's `libatomic.a` is a comment saying the family moved into
`libclang_rt.builtins-*.a`, so the `-latomic` the target spec already passes
resolves nothing. `:app:rustBuildX86Release` failed on undefined
`__atomic_load_8` and took v2.0.0-alpha.5 with it.

`crates/copypaste-ui/src-tauri/build.rs` asks the Android compiler for
`-print-libgcc-file-name` and puts that archive on the link line, for
`target_arch = "x86"` alone. It is a build script and not `.cargo/config.toml`
because the Tauri build sets `CARGO_ENCODED_RUSTFLAGS` — measured, to
`-landroid -llog -lOpenSLES` — and that outranks `target.<triple>.rustflags`
outright, so a config entry would have been read past in silence.

## Alternatives rejected

**Vendor OpenSSL on every target.** One backend everywhere, and it needs no
crate manifest change, so it was tempting. Rejected: it would move macOS off a
free, OS-provided, Apple-maintained backend onto a several-minute build and a
larger binary, for no user-visible gain, and would put a C OpenSSL build into
the macOS product — the thing AGENTS.md's third exemption is about.

**Cross-compile OpenSSL in the workflow and point `OPENSSL_LIB_DIR` at it.**
Rejected on rule 1: it is a hand-rolled reimplementation of `openssl-src`, in
YAML, times four ABIs. It also links dynamically, which would mean packaging
`libcrypto.so` into the APK's `jniLibs`.

**Drop SQLCipher on Android.** Not considered seriously. The database is
encrypted at rest on both platforms or the product is not what it claims.

## Unverified

The Android OpenSSL build has compiled but never installed. The fourth release
dry run got through the `aarch64-linux-android` compile and failed on ranlib;
the three other ABIs have not been reached at all, and no run has yet linked
`sqlite3.c` against the result. None of this can be reproduced in the
development container: there is no NDK, `dl.google.com` is unreachable, and the
one available cross-compilation trick — a shim that emits an empty object file —
would fake exactly the C compilation being tested. The ranlib wiring is
verified only to the extent that the emitted variable names are asserted and
`cc`'s precedence order was read from the vendored source.

The macOS linkage check has likewise never run on macOS; `otool` does not exist
in the container. Whether it *passes* is the open question, and it is the
question worth answering: if macOS has been linking Homebrew's libcrypto all
along, the check failing is the bug being found, not the check being wrong.
