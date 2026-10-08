# Hidden UI memory

The main application unloads its presentation after 60 continuous seconds with
no visible main window. Minimizing and closing to the tray count as hidden;
losing keyboard focus while the window stays visible does not. Flutter's hidden
and paused lifecycle states provide the equivalent behavior on Android.

`ApplicationVisibility` combines the native desktop window and Flutter lifecycle
signals. `UiMemoryController` owns the deadline. `CopyPasteRoot` retains the
runtime, capture/notification services, tray, global shortcut, and feature
controllers while removing the application widget tree. Quick Paste keeps its
independent presentation lifecycle.

History discards loaded pages, selected detail payloads, previews, source icons,
and facets. Its query, selected item identity, and collapsed sections remain.
Runtime events do not reload History while it is suspended. Query and media
responses from before suspension cannot refill the released state. Reopening
rebuilds the screens and loads current History using the saved query. Settings
search and category selection also survive unloading.

An open overlay or detail route, pairing, onboarding, or a user operation delays
unloading until another hidden deadline. This preserves unsaved input and active
work. Returning to a visible window cancels the deadline immediately.

Flutter normally disables frames while hidden. A single warm-up frame disposes
the screens; its post-frame callback clears decoded images after image listeners
have detached. There is no recurring rendering while suspended.

Tests cover the deadline, cancellation, restoration, main-window visibility when
Quick Paste activates the app, stale asynchronous responses, controller ownership,
and decoded-cache bytes before and after unloading. Cached image bytes measure
released cache ownership, not total process RAM. Native physical footprint needs
a release build measured before/after the same image workload; Flutter engine and
allocator memory can remain resident even after application references are gone.
