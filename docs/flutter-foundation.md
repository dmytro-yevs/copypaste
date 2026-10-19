# Shared Flutter architecture

The application lives in `apps/copypaste_flutter`. Its shared feature controllers
and repositories own business logic and state; typed adapters isolate operating
system integration. The Rust bridge connects the shared interface to the runtime.
Regenerate both sides of the bridge from `flutter_rust_bridge.yaml` when its API
changes, keeping the generated protocol hashes identical.

Use the pinned `shadcn_flutter` components with `AppTheme`, shared tokens, and
`AppMotion`. Application buttons use the shadcn `Button` API. Reuse the same
controls, state, and actions across macOS, Windows, Android, and supported Linux
layouts. Source application identity belongs in `SourceAppLabel`.

Inspector actions stay in a fixed footer outside the scrollable body. Metadata
scrolls when it overflows; short metadata may fit without scrolling. Image
viewports that the layout marks as fixed remain outside that scrollable body.
Dialogs follow keyboard insets through the shared motion policy and respect
reduced-motion preferences.

Loading, empty, and error presentations use the shared state view. Expensive
services, models, streams, and retained payloads have explicit owners and bounded
lifetimes. Tests of Flutter counters do not establish released native memory.

See [development](development.md) and
[production qualification](release-qualification.md).
