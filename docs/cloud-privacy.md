# Cloud synchronization privacy

Cloud synchronization is supplied by the optional signed Supabase module.
Clipboard content is encrypted on the client with a passphrase-derived key;
credentials and derived synchronization keys belong in application-owned
encrypted state. The backend can observe account identifiers, connection and
traffic information, and synchronized metadata, including producer privacy
classification. Do not describe the backend as learning no metadata.

Authenticated merge metadata and client-side encryption do not establish the
security of a deployed service. Repository SQL and local HTTP/PostgreSQL fixtures
cannot prove a deployment's row-level security, authentication, realtime settings,
or retention configuration; qualify that deployment separately.

Peer revocation prevents subsequent synchronization locally. It does not erase
history already delivered to another device or automatically rotate all keys.
Avoid recording plaintext clipboard content, credentials, or derived keys in
logs and diagnostic artifacts. The application should expose actionable errors
without leaking sensitive payloads or local paths.

See the repository's [security contract](../SECURITY.md) and
[release qualification](release-qualification.md).
