# Decisions and studies

One line each: the question the document settles. Read the document only if the
answer surprises you.

## Decided

| | Question it settles |
|---|---|
| [ADR-0001](adr/0001-macos-distribution-without-a-developer-id.md) | We do not buy a Developer ID. So: ad-hoc signing, our own Homebrew tap, and an app that must need **no** TCC permission, because a grant would be tied to a code hash that changes every build. |
| [ADR-0002](adr/0002-one-cross-platform-app.md) | One Tauri v2 + React app on macOS and Android, not a SwiftUI app and a Compose app. Reverses the native decision taken earlier the same day; the deleted work is at `2cbeef3b`. |
| [ADR-0003](adr/0003-one-command-surface-two-backends.md) | How the Tauri bridge reaches the core on each platform: one `Backend` trait, a daemon impl and an in-process impl, chosen by a compile-time alias so `commands/` contains no `cfg`. |
| [android-clipboard-access](rewrite/android-clipboard-access.md) | What Android lets a clipboard manager read, and what we ask the user for. A four-rung ladder with rung 0 (no permission) as the default and Shizuku as the one upgrade we build; v1's `READ_LOGS` approach is not ported. |
| [target-architecture](rewrite/target-architecture.md) | Which maintained crate does each job, and the short list of custom code that stays — with the reason each one is not replaceable. |
| [design/README](../design/README.md) | The visual system: shadcn/ui on Tailwind v4, zinc base in OKLCH, one token source in `design/tokens/`, and a contrast gate that measures 636 composited pairs rather than asserting AA. |
| [supabase-deployment](supabase-deployment.md) | What the server side of cloud sync is: one table, its RLS policies, its Realtime publication, and the retention job — plus where the deployment deviates from the contract in `copypaste-cloud/src/rest/mod.rs`. |
| [cloud-privacy](cloud-privacy.md) | What the backend can see when cloud sync is on, what it cannot, and the two things that got worse than v1's account-less relay: an email address plus a metadata surface, and an account password that reaches the rows without decrypting them. |

## Measured

| | What it establishes |
|---|---|
| [parity-audit](rewrite/parity-audit.md) | What v0.4.1 did that v2 does not: nineteen capabilities that were neither ported nor recorded as dropped, ranked by what a user loses, each with where the auditor looked. |
| [security-review](rewrite/security-review.md) | Whether `SECURITY.md`'s claims hold against the code. Fourteen findings, two High, and a list of the properties that did hold. |
| [e2e/README](../e2e/README.md) | What a green real-WebView run proves and what it does not — the engine is WebKitGTK, so it is evidence about Linux, not about either shipping platform. |

## Binding

| | |
|---|---|
| [CLAUDE.md](../CLAUDE.md) | The working rules. Dependencies are the default; no v1 compatibility; 500 lines a file; a feature is not done until it has a UI. |
| [port-manifest/README](rewrite/port-manifest/README.md) | Which manifest sections are still requirements and which became reference material. Behaviour binds; the v1 byte formats and the v1 visuals do not. |
| [design-reference.html](rewrite/design-reference.html) | v1's visual reference. Historical — superseded by `design/`. |
