/**
 * The capture surfaces author no sentence about *state*.
 *
 * `CaptureSnapshot` arrives with a finished `headline` and `detail` — decided in
 * `capture::messages`, which is compiled and tested (ADR-0005) — and the view
 * renders them verbatim. Everything here is chrome the state machine has no
 * opinion about: section titles, the names of the setup steps, and the label on
 * whatever button the snapshot's `nextStep` chose.
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
      title: "Saving from this app",
      body: "CopyPaste can read the clipboard whenever it is the app in front. This needs no permission and cannot lapse.",
      action: "Save the clipboard now",
      saved: "Saved to your history",
      nothing: "There was nothing on the clipboard to save",
    },

    ladder: {
      title: "Capturing from other apps",
      label: "Setup steps",
      install: "Install Shizuku",
      start: "Start Shizuku",
      permission: "Give CopyPaste permission in Shizuku",
      armed: "Turn on background capture",
      /** Each step says where it stands in words as well as in a glyph — a
       *  checklist that is only a colour is not a checklist (A11Y-10). */
      done: "Done",
      next: "Next",
      todo: "Not done yet",
    },

    action: {
      arm: "Turn on background capture",
      permission: "Ask Shizuku for permission",
      /** Neither installing nor starting Shizuku is something CopyPaste can do,
       *  so the only honest button on those steps is the one that re-reads. */
      checkAgain: "Check again",
      busy: "Working…",
    },

    enable: {
      title: "Capture from other apps",
      body: "Turning this off leaves everything above running.",
    },

    /** A copy that was taken and not stored is the failure this whole feature
     *  is about, so it is stated rather than logged. */
    dropped_one: "{{count}} copy was captured but couldn't be saved.",
    dropped_other: "{{count}} copies were captured but couldn't be saved.",

    lastSaved: "Last saved {{age}}",
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
