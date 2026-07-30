# Decisions and studies

One line each: the question the document settles. Read the document only if the
answer surprises you.

## Decided

| | Question it settles |
|---|---|
| [ADR-0001](adr/0001-macos-distribution-without-a-developer-id.md) | We do not buy a Developer ID. So: ad-hoc signing, our own Homebrew tap, and an app that must need **no** TCC permission, because a grant would be tied to a code hash that changes every build. |
| [ADR-0002](adr/0002-one-cross-platform-app.md) | One Tauri v2 + React app on macOS and Android, not a SwiftUI app and a Compose app. Reverses the native decision taken earlier the same day; the deleted work is at `2cbeef3b`. |
| [ADR-0003](adr/0003-one-command-surface-two-backends.md) | How the Tauri bridge reaches the core on each platform: one `Backend` trait, a daemon impl and an in-process impl, chosen by a compile-time alias so `commands/` contains no `cfg`. |
| [ADR-0004](adr/0004-the-app-owns-the-daemon.md) | On macOS the app starts and stops the daemon, and installs no launchd agent — because two supervisors over one socket is the failure. A daemon it merely found running is adopted read-only. |
| [ADR-0005](adr/0005-android-capture-in-rust-kotlin-reports.md) | Kotlin reports facts; Rust decides what they mean. The capture ladder's state machine and every sentence it shows compile and are tested on a host with no Android SDK. |
| [ADR-0006](adr/0006-android-release-signing.md) | What the released APK is signed with, and why an unsigned one is not an option the way an ad-hoc `.app` is. |
| [ADR-0007](adr/0007-sqlcipher-crypto-backend-per-platform.md) | Which crypto provider SQLCipher is compiled against on each platform, and what the build must guarantee about the linkage. |
| [android-clipboard-access](rewrite/android-clipboard-access.md) | What Android lets a clipboard manager read, and what we ask the user for. A four-rung ladder with rung 0 (no permission) as the default and Shizuku as the one upgrade we build; v1's `READ_LOGS` approach is not ported. |
| [target-architecture](rewrite/target-architecture.md) | Which maintained crate does each job, and the short list of custom code that stays — with the reason each one is not replaceable. |
| [design/README](../design/README.md) | The visual system: shadcn/ui on Tailwind v4, zinc base in OKLCH, one token source in `design/tokens/`, and a contrast gate that measures composited pairs rather than asserting AA. |
| [supabase-deployment](supabase-deployment.md) | What the server side of cloud sync is: one table, its RLS policies, its Realtime publication, and the retention job — plus where the deployment deviates from the contract in `copypaste-cloud/src/rest/mod.rs`. |
| [cloud-privacy](cloud-privacy.md) | What the backend can see when cloud sync is on, what it cannot, and the two things that got worse than v1's account-less relay: an email address plus a metadata surface, and an account password that reaches the rows without decrypting them. |

## Measured

| | What it establishes |
|---|---|
| [parity-audit](rewrite/parity-audit.md) | What v0.4.1 did that v2 does not: nineteen capabilities that were neither ported nor recorded as dropped, ranked by what a user loses, each with where the auditor looked. |
| [ui-parity-audit](rewrite/ui-parity-audit.md) | What v1's interface showed that v2's does not — twelve surfaces, nine of them readouts rather than actions. The capability question is the parity audit's; this one is what the screen said. |
| [security-review](rewrite/security-review.md) | Whether `SECURITY.md`'s claims hold against the code — the findings, and the list of properties that did hold. Dated; each finding carries its status in place. |
| [claims-audit](rewrite/claims-audit.md) | Which statements anywhere in this repository the code contradicts, and the one shape most of them share: an inventory of absence that nothing falsifies when the absence ends. |
| [android-spike](rewrite/android-spike.md) | What a first run on real Android hardware would falsify, in the order to expect it. Nothing under `gen/android/` has ever been compiled. |
| [post-merge-review](rewrite/post-merge-review.md) | What a green suite did not catch in the ~37 commits of 2026-07-30, read against a clean checkout rather than the working tree. |
| [e2e/README](../e2e/README.md) | What a green real-WebView run proves and what it does not — the engine is WebKitGTK, so it is evidence about Linux, not about either shipping platform. |
| [backlog](backlog.md) | Everything still outstanding, in one ranked list: the two parity audits, the security review, the ADRs' open questions and the code's own refusals, deduplicated and re-checked against the tree — plus what has already closed, what is a decision rather than a debt, and what is waiting on hardware. |

## Binding

| | |
|---|---|
| [CLAUDE.md](../CLAUDE.md) | The working rules. Dependencies are the default; no v1 compatibility; 500 lines a file; a feature is not done until it has a UI. |
| [port-manifest/README](rewrite/port-manifest/README.md) | Which manifest sections are still requirements and which became reference material. Behaviour binds; the v1 byte formats and the v1 visuals do not. |
| [design-reference.html](rewrite/design-reference.html) | v1's visual reference. Historical — superseded by `design/`. |
