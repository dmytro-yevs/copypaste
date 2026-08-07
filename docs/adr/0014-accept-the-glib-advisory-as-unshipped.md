# ADR-0014 — Accept the glib advisory as an unshipped subtree

**Status:** accepted · 2026-08-07
**Scope:** the Dependabot alert on `glib 0.18.5` (RUSTSEC-2024-0429).

## Decision

Accept it. Do not upgrade Tauri for it, do not force the dependency out of the
lock, and stop Dependabot retrying an update it cannot produce:
`.github/dependabot.yml` ignores `glib` and nothing else.

## The exposure

`Cargo.lock` pins `glib 0.18.5`. The lowest non-vulnerable version is `0.20.0`;
the highest this tree resolves is `0.18.5`, so Dependabot fails with
`security_update_not_possible` on every run and reports no conflicting
dependency to fix.

Nothing in the tree can move it alone. `glib 0.18.5` is held by the whole GTK
0.18 stack — `gtk 0.18.2`, `webkit2gtk 2.0.2`, `gdk`, `gdk-pixbuf`, `gdkx11`,
`atk`, `cairo-rs`, `pango`, `soup3`, `javascriptcore-rs`, `libappindicator` —
which Tauri pulls in for its Linux desktop webview.

## Why it is not shipped

Linux desktop is not a shipping target under CLAUDE.md rule 7. It is a test
surface: `browser-webkitgtk.yml` drives the shared React frontend through
WebKitGTK under Xvfb because that is the only engine available here with a
layout, and the workflow's own header says nothing it observes is evidence
about a shipping platform. No macOS or Android target resolves the GTK stack at
all; `deny.toml`'s `[graph]` carries the Linux triple precisely so `cargo deny`
still sees what that one CI job builds.

So the reachable exposure is an unsound `VariantStrIter` path inside a runner
that already executes arbitrary frontend code. It is not on a user's device and
not on the crypto, storage or IPC path.

ADR-0013 (Windows as a third platform) is landing in parallel and does not
change this: it amends rule 7 to three shipped platforms and keeps Linux
desktop excluded, and Windows has no code in the tree to resolve GTK from.

## Consequences

- **`glib` alone is silenced.** Every other crate still raises Dependabot
  alerts and still gets security update PRs, and `supply-chain.yml` gates every
  push unchanged.
- **A future `glib` advisory will not be reported by Dependabot either.**
  Dependabot's `ignore` is keyed on the dependency, not the advisory, and there
  is no advisory-level scope to key on. `cargo audit` and `cargo deny` are a
  separate list and do not carry this one — `deny.toml`'s note records why the
  RUSTSEC entry had to be removed from it.
- The alert stays open in the Security tab. Dismissing it there is a separate
  and reversible act, and is not what this decides.

## What would change this

Tauri's Linux stack moving to `glib 0.20+` — most plausibly when `tray-icon`
migrates off GTK3, which is the same event that clears the eight
unmaintained-GTK ignores in `deny.toml`. Remove the `dependabot.yml` entry in
that commit; an ignore nobody deletes is how the next real advisory gets
missed.

Linux desktop becoming a shipping target would also change it, and would make
the upgrade mandatory rather than optional.
