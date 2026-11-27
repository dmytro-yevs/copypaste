# Security

[Русский](SECURITY.ru.md)

Report vulnerabilities through the repository's private security advisory channel.
Include the affected version and platform, reproduction steps using synthetic data,
and the impact.

CopyPaste encrypts stored history, keeps keys in the platform keystore, and respects
private and confidential clipboard markers when the operating system provides them.
Pairing requires both devices to confirm the same code. Modules are trusted native
extensions and are not sandboxed.

OCR and semantic-search inference run locally. Clipboard history synchronizes
only with paired devices or, when enabled, through Supabase Sync. Supabase Sync
encrypts content before upload. In Copy file storage mode, files of up to 64 MiB
are also uploaded as encrypted objects with opaque names and deleted after
24 hours.
