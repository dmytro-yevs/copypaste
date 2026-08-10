# ADR-0021: Accept the glib advisory as unshipped

Status: accepted
Owner: `@dmytro-yevs`
Expires: 2026-11-10

## Decision

Accept `glib 0.18.5` under RUSTSEC-2024-0429 only until the expiry above. The
affected `VariantStrIter` methods are not called in the workspace or resolved
dependency sources, and the crate resolves only for Linux desktop. Linux is a
WebKitGTK browser-test surface; macOS, Android and Windows do not resolve it.

The exact Linux chain includes `copypaste-ui → tauri 2.11.5 →
tauri-runtime-wry 2.11.4 → wry 0.55.1 → webkit2gtk 2.0.2 → glib 0.18.5`.
Tauri's tray and event-loop paths also reach the same GTK3 resolution.

Keep Dependabot enabled for `glib`. `scripts/check_rustsec_policy.py` consumes
`cargo-audit` JSON and fails unless the advisory, alias, package, version, kind,
owner, decision and unexpired date match `config/rustsec-exceptions.json`. A
missing advisory also fails so remediation cannot leave a stale exception.
The same check requires the crate on Linux and its absence on every configured
macOS, Android and Windows architecture.

## Upstream constraint

[Tauri 2.11.5](https://crates.io/crates/tauri/2.11.5) is the current maintained
release and constrains Tauri Runtime Wry to Wry 0.55. [Wry
0.56.0](https://crates.io/crates/wry/0.56.0), although newer, still depends on
GTK3 0.18 and [webkit2gtk 2.0.2](https://crates.io/crates/webkit2gtk/2.0.2),
which requires `glib ^0.18`. The fixed `glib 0.20` line therefore cannot enter
this graph without replacing or patching the upstream GTK/WebKit stack.

No maintained package adds owner- and expiry-bound decisions to informational
`cargo-audit` findings: `cargo-audit` reports this advisory as `unsound`, while
`cargo-deny` v2 does not raise it. The policy wrapper is exemption 1 from
AGENTS.md; it delegates advisory discovery to those maintained tools and only
validates their JSON against the decision metadata.

## Consequences

The alert is accepted risk, not fixed. It must remain open or be dismissed as
tolerated risk, never closed as remediated. Any other vulnerability,
unsoundness or notice fails CI; the self-test seeds an unapproved advisory to
prove that path.

Remove the exception when Tauri resolves `glib 0.20` or later. Renewing the
date requires explicit review of upstream releases and this ADR; shipping Linux
desktop makes the exception invalid immediately.
