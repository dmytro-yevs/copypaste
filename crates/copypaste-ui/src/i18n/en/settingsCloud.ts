export const settingsCloud = {
  title: "Cloud sync",
  sectionTitle: "Connection and account",
  sectionDescription:
    "Encrypted cloud history is optional. Direct device sync keeps working without it.",
  connectionTitle: "Cloud connection",
  description: "Syncs encrypted history through your configured account.",
  notConfigured: "No cloud server is configured. Direct device sync still works.",
  badgeUnavailable: "Unavailable",
  badgeNotConfigured: "Not configured",
  badgeConnected: "Connected",
  badgeSignedOut: "Signed out",
  loading: "Checking account…",
  statusUnavailable: "Cloud account status couldn't be loaded.",
  retry: "Try again",
  lastError: "The last cloud sync failed. Try again or sign in again.",
  // The item content and identity deliberately never reach the client, but the
  // count distinguishes a complete upload from one that is still retrying.
  unreadableUploads_one:
    "1 item on this device could not be prepared for cloud sync and is being retried.",
  unreadableUploads_other:
    "{{count}} items on this device could not be prepared for cloud sync and are being retried.",
  formLabel: "Cloud account sign in",
  accountTitle: "Cloud account",
  accountConnectedDescription: "Your encrypted history can sync through this account.",
  accountSignedOutDescription: "Sign in to an existing account or create a new one.",
  email: "Email",
  password: "Password",
  passphrase: "Sync passphrase",
  passphraseHelp:
    "Use at least 12 characters. It encrypts clipboard data on this device before upload.",
  signIn: "Sign in",
  signingIn: "Signing in…",
  createAccount: "Create account",
  signingUp: "Creating account…",
  signInError: "Sign-in failed. Check the credentials and try again.",
  signUpError: "The account couldn't be created. Check the details and try again.",
  credentialsUnsaved: "Unsaved cloud credentials",
  discard: "Discard",
  cancel: "Cancel",
  signOut: "Sign out",
  signingOut: "Signing out…",
  signOutError: "Cloud sign-out failed. Try again.",
  syncNow: "Sync cloud now",
  syncing: "Syncing…",
  syncError: "Cloud sync failed. Check the connection and try again.",
  endpoint: {
    title: "Cloud server",
    description: "Connect a compatible server before adding a cloud account.",
    configuredDescription:
      "Server credentials are stored on this device and are never shown again.",
    formLabel: "Cloud server configuration",
    url: "Server URL",
    publishableKey: "Publishable key",
    help:
      "Use the server URL and its publishable anon key. The key identifies the server; it is not your account password. Direct device sync remains independent.",
    changeWarning:
      "Changing or restoring the cloud server signs this device out. Local clipboard history and direct device sync are not changed.",
    restoreHelp:
      "If this build has no hosted service, restoring the default returns cloud sync to Not configured.",
    configure: "Configure",
    change: "Change server",
    save: "Save server",
    restore: "Restore hosted default",
    saving: "Saving cloud server…",
    error: "The cloud server couldn't be changed. Check the URL and key, then try again.",
    unsaved: "Unsaved server credentials",
  },
  toast: {
    signedIn: "Cloud sync connected",
    signedOut: "Signed out of cloud sync",
    synced: "Cloud sync finished: {{uploaded}} uploaded, {{downloaded}} downloaded",
    syncedWithSkips:
      "Cloud sync finished: {{uploaded}} uploaded, {{downloaded}} downloaded, {{skipped}} skipped",
  },
} as const;
