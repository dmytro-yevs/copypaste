# Supabase Sync

Supabase Sync is an optional module for encrypted clipboard synchronization.
Install it from **Settings → Modules**, enter your Supabase project URL and
publishable key, then sign in and set a sync passphrase. Use the same account
and passphrase on every device.

Your passphrase stays on the device. Disabling the module stops synchronization;
removing it clears its account data but keeps local clipboard history.

With **File storage → Keep a copy** in Settings, files you copy (up to 64 MiB)
are also uploaded encrypted, so your other devices can download them while this
device is offline. They are deleted from Supabase after 24 hours, when the clip
is deleted, or when you sign out.
