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
    title: "Know when a copy is saved",
    body: "CopyPaste can ping you when a copy is saved in the background. Clipboard history does not need this permission, and you can skip it.",
    bodyWindows:
      "Windows does not show a permission prompt for notices. Turn them on here, or skip — history still works.",
    bodyDenied:
      "Notifications were not allowed. Try again, or open Settings. CopyPaste still saves your history either way.",
    action: "Allow notifications",
    actionEnable: "Turn on save notices",
    actionRetry: "Try again",
    actionContinue: "Continue",
    openSettings: "Open Settings",
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
    body: "CopyPaste already saves what you copy in this app, share to it, or capture with a Quick Settings tile. Background capture from every app is optional and can wait.",
    action: "Continue",
    saveNow: "Save the clipboard now",
    addTile: "Add Quick Settings tile",
    tileAdded: "Quick Settings tile added",
    background: "Turn on background capture",
  },

  settings: {
    title: "Show setup again",
    description: "Opens the first-run steps. Your clipboard history is not changed.",
    action: "Show setup",
  },
} as const;
