/**
 * No sentence here describes capture *state*. `CaptureSnapshot` arrives with a
 * finished `headline` and `detail` from `capture::messages`, which is compiled
 * and tested (ADR-0005), and the view renders them verbatim.
 */
export const capture = {
  title: "Background capture",

  loading: {
    title: "Checking…",
    body: "Asking this device what CopyPaste is allowed to capture.",
  },

  /** The state is unknown, which is a different thing from "not capturing".
   *  Nothing claims a rung here (CopyPaste-qzhu). */
  unknown: {
    title: "CopyPaste can't tell what it is capturing",
    body: "This build didn't answer when asked about background capture. Everything you copy inside CopyPaste is still saved.",
  },

  status: {
    /** `{{summary}}` is the state machine's own sentence, so a pointer user and
     *  a screen reader user get the same words. The dot is decorative. */
    label: "Background capture: {{summary}}",
    open: "Set up",
    openHint: "Open background capture setup",
  },

  setup: {
    always: {
      saved: "Saved to your history",
      nothing: "There was nothing on the clipboard to save",
    },

  },

  options: {
    title: "Android clipboard notice",
  },

  toast: {
    row: {
      title: "Hide Android's clipboard notice",
      body: "Android announces each time something reads your clipboard.",
    },
    dialog: {
      title: "Turn off Android's clipboard notice?",
      confirm: "Turn the notice off",
      loading: "Loading what this changes…",
      /** Shown *instead of* the confirm button. There is nothing to consent to
       *  until the explanation is on screen, so the button is absent rather
       *  than disabled. */
      unavailable:
        "CopyPaste can't show what this changes right now, so it won't change it. Try again in a moment.",
    },
  },
} as const;
