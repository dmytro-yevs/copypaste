/**
 * The runtime-event viewer's strings.
 *
 * `loadFailed` is the app's own sentence for a viewer that could not read the
 * log. The service authors its own copy of it (`commands::diagnostics`), which
 * arrives classified rather than as text, so the two never reach the DOM
 * together.
 */
export const runtimeLog = {
  title: "Runtime events",
  list: "Runtime event list",

  search: {
    label: "Search runtime events",
  },

  level: {
    label: "Level",
    all: "All levels",
    error: "Errors",
    warn: "Warnings",
    info: "Info",
    debug: "Debug",
    trace: "Trace",
  },

  process: {
    label: "Process",
    all: "All processes",
    app: "App",
    daemon: "Service",
  },

  refresh: "Refresh runtime events",
  pause: "Pause live updates",
  resume: "Resume live updates",
  copyLoaded: "Copy loaded log events",
  copyEntry: "Copy log entry",

  loading: "Loading runtime events…",
  loadingOlder: "Loading older events…",
  loadFailed: "Runtime events couldn’t be loaded.",
  /** Said of the live tail only: the loaded rows are still on screen, and they
   *  have stopped being current. */
  followFailed: "Live updates stopped. These events may be out of date.",
  noMatch: "No runtime events match these filters.",

  toast: {
    entryCopied: "Log entry copied",
    entryCopyFailed: "Couldn’t copy this log entry.",
    loadedCopied: "Loaded log events copied",
    loadedCopyFailed: "Couldn’t copy loaded log events.",
  },
} as const;
