# Supabase Sync

An optional first-party `copypaste.supabase` module for macOS, Android, and
Windows. CopyPaste works locally and over P2P without this package. The native module
contains no local database or OS keystore implementation; the application
provides those services through bounded callbacks.

Install the signed `.cpmodule` through Settings > Modules. In the module settings,
set your Supabase project URL and publishable key under Preferences, enable the
module, then run Sign in or Create account. Use the same account and sync
passphrase on every device. The passphrase must contain at least 12 characters;
it never goes to Supabase. Password and passphrase fields are transient.
If your project requires email confirmation, confirm the email before signing in.

Supabase receives encrypted rows and signed metadata. Session tokens and the
derived sync key stay in CopyPaste's encrypted database. Disabling stops workers
and keeps the account for re-enabling. Removing clears module state and package
data; local clipboard history remains available. The global Synchronization
setting also stops the module.

Provision the backend using [the maintained schema](../../supabase/) and
[deployment instructions](../../docs/supabase-deployment.md). Do not use a
`service_role` key. No endpoint or account is bundled into the application.

Build the independent library:

```sh
cargo +1.96 build --manifest-path modules/supabase/Cargo.toml --locked --release
```

Package it with `scripts/modules/package.py` and the existing release signer.
`.github/workflows/supabase-module.yml` builds and signs all five targets;
publication to the signed Marketplace remains a separate operation.

The signed native integration scenario uses temporary encrypted histories,
a temporary signing identity, and a local HTTP fixture:

```sh
./scripts/demo-cloud.sh
```

Only this scenario enables `test-endpoints`. Shipped packages accept HTTPS/WSS
only. Local fixture tests establish native module boundaries and convergence;
they do not establish live Supabase deployment or physical device qualification.
