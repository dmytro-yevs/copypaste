# ADR-0021: Accept the glib advisory as unshipped

Status: accepted

## Decision

Accept `glib 0.18.5` under RUSTSEC-2024-0429 and ignore `glib` alone in
Dependabot. Do not force the crate from the lock or upgrade Tauri solely for
this alert.

The lowest non-vulnerable release is `0.20.0`, while this tree resolves at most
`0.18.5`, so Dependabot reports `security_update_not_possible`. The crate
belongs to Tauri's GTK/WebKitGTK Linux desktop subtree. Linux desktop is a CI
browser test surface, not a shipping target; macOS, Android and Windows do not
ship this dependency path.

## Consequences

Dependabot cannot ignore one advisory for a dependency, so future `glib`
advisories are also muted there. `cargo audit` and `cargo deny` continue to
gate the full dependency graph, and every other crate remains eligible for
Dependabot security updates.

The existing Security-tab alert remains open. This decision stops the
unresolvable update retry; it does not dismiss the alert.

Remove the ignore when Tauri's Linux stack can resolve `glib 0.20` or later.
If Linux desktop becomes a shipping target first, upgrading becomes mandatory.
