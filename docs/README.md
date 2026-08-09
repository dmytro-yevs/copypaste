# Decisions and specifications

## Decided

| | Question it settles |
|---|---|
| [ADR-0001](adr/0001-macos-distribution-without-a-developer-id.md) | We do not buy a Developer ID. So: ad-hoc signing, our own Homebrew tap, and an app that must need **no** TCC permission, because a grant would be tied to a code hash that changes every build. |
| [ADR-0002](adr/0002-one-cross-platform-app.md) | One Tauri v2 + React app on macOS and Android. |
| [ADR-0003](adr/0003-one-command-surface-two-backends.md) | How the Tauri bridge reaches the core on each platform: one `Backend` trait, a daemon impl and an in-process impl, chosen by a compile-time alias so `commands/` contains no `cfg`. |
| [ADR-0004](adr/0004-the-app-owns-the-daemon.md) | On macOS the app starts and stops the daemon, and installs no launchd agent — because two supervisors over one socket is the failure. A daemon it merely found running is adopted read-only. |
| [ADR-0005](adr/0005-android-capture-in-rust-kotlin-reports.md) | Kotlin reports facts; Rust decides what they mean. The capture ladder's state machine and every sentence it shows compile and are tested on a host with no Android SDK. |
| [ADR-0006](adr/0006-android-release-signing.md) | What the released APK is signed with, and why an unsigned one is not an option the way an ad-hoc `.app` is. |
| [ADR-0007](adr/0007-sqlcipher-crypto-backend-per-platform.md) | Which crypto provider SQLCipher is compiled against on each platform, and what the build must guarantee about the linkage. |
| [ADR-0019](adr/0019-decode-realtime-jwt-subjects-without-verification.md) | Realtime JWT subjects are decoded only to form the server-verified subscription filter. |
| [ADR-0020](adr/0020-windows-distribution-and-update-signing.md) | Windows ships through current-user NSIS with separate Authenticode and updater signatures. |
| [ADR-0021](adr/0021-accept-the-glib-advisory-as-unshipped.md) | The glib advisory is accepted only for the unshipped Linux test subtree. |
| [android-clipboard-access](rewrite/android-clipboard-access.md) | What Android lets a clipboard manager read, and what we ask the user for. A four-rung ladder with rung 0 (no permission) as the default and Shizuku as the one upgrade we build; v1's `READ_LOGS` approach is not ported. |
| [target-architecture](rewrite/target-architecture.md) | Which maintained crate does each job, and the short list of custom code that stays. |
| [design/README](../design/README.md) | The visual system: shadcn/ui on Tailwind v4, zinc base in OKLCH, one token source in `design/tokens/`, and a contrast gate that measures composited pairs rather than asserting AA. |
| [supabase-deployment](supabase-deployment.md) | What the server side of cloud sync is: one table, its RLS policies, its Realtime publication, and the retention job — plus where the deployment deviates from the contract in `copypaste-cloud/src/rest/mod.rs`. |
| [cloud-privacy](cloud-privacy.md) | What the backend can see when cloud sync is on, what it cannot, and the two things that got worse than v1's account-less relay: an email address plus a metadata surface, and an account password that reaches the rows without decrypting them. |

## Requirements and operating guides

| | What it establishes |
|---|---|
| [android-clipboard-access](rewrite/android-clipboard-access.md) | Android's clipboard-access limits and the supported capture ladder. |
| [android-spike](rewrite/android-spike.md) | Platform checks for a real Android device run. |
| [testing-policy](rewrite/testing-policy.md) | The authoritative verification layer for each requirement. |
| [performance](rewrite/performance.md) | Performance measurements and how to reproduce them. |
| [e2e/README](../e2e/README.md) | The browser-layer suite and its limits. |

## Binding

| | |
|---|---|
| [CLAUDE.md](../CLAUDE.md) | The working rules. |
| [port-manifest/README](rewrite/port-manifest/README.md) | Which manifest sections are still requirements and which became reference material. Behaviour binds; the v1 byte formats and the v1 visuals do not. |
