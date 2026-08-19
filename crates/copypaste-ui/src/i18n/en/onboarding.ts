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
    body: "Show or scan a pairing code. Codes stay in CopyPaste's protected view. Skip if you only use this device.",
    action: "Continue",
  },

  cloud: {
    title: "Sync over the internet",
    body: "Optional. This device works on its own. Open Advanced only for a self-hosted project.",
    hosted:
      "Sign in or create an account. CopyPaste already knows the cloud — you don't paste a project URL or key.",
    connected: "Cloud sync is on for this device.",
    action: "Continue",
    signIn: "Sign in",
    createAccount: "Create account",
    advanced: "Advanced — self-hosted project",
    advancedForm: "Self-hosted cloud endpoint",
    url: "Project URL",
    anonKey: "Anon key",
    saveEndpoint: "Use this project",
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
