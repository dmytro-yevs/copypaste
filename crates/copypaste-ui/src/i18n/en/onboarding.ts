export const onboarding = {
  progress: "Step {{current}} of {{total}}",
  back: "Back",
  skip: "Not now",
  continue: "Continue",
  skipSetup: "Skip setup",

  welcome: {
    title: "Welcome to CopyPaste",
    body: "A few short steps so CopyPaste can save what you copy. Skip anything you don't need.",
    action: "Get started",
  },

  done: {
    title: "You're ready",
    body: "CopyPaste will keep what you copy on this device. Pair other devices or use cloud sync later from Settings.",
    action: "Start using CopyPaste",
  },

  permissions: {
    title: "Allow clipboard access",
    body: "CopyPaste needs permission to save what you copy. The next screen is the system prompt. You can skip this and turn it on later.",
    action: "Continue",
  },

  pairing: {
    title: "Add another device",
    body: "Pair a phone or computer to share your clipboard between them. You can do this later from Devices.",
    action: "Continue",
  },

  cloud: {
    title: "Sync over the internet",
    body: "Optional. Sign in later from Settings if you want a backup when devices aren't on the same network.",
    action: "Continue",
  },

  capture: {
    title: "Capture from other apps",
    body: "Optional. Needs a one-time setup and a re-tap after each restart. Items synced from your other devices already work.",
    action: "Continue",
  },

  settings: {
    title: "Show setup again",
    description: "Opens the first-run steps. Your clipboard history is not changed.",
    action: "Show setup",
  },
} as const;
