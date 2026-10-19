# Production release qualification

The production pipeline is defined in
[release.yml](../.github/workflows/release.yml). A stable `v<version>` tag must name
the exact source commit used to build, qualify, sign, recover, and publish a
release. Publication consumes the qualified bytes and their receipts; caches are
not release evidence. Never move a published tag or replace conflicting assets.

Before advancing the application version, advance the OCR, Semantic Search, and
Supabase module versions and bind their minimum application version to that
release. Keep the Rust workspace, Flutter build name, build number, and Android
version code consistent with their release contracts. Update the release notes.

Run the portable workflow, security, policy, release safety, and qualification
regression gates before publishing. Native qualification must verify platform
identity, artifact digest, architecture, signer, and the maintained behavior
matrix. All required platform and qualification jobs must succeed before the
publisher runs. Manual qualification runs do not publish by default.

The pipeline produces a sealed macOS DMG with its bundled daemon, signed Android
universal/ARM APKs, a signed Windows installer, and the declared Linux packages.
Linux's stable scope and exact-artifact scenarios are specified in the
[support matrix](../packaging/linux/support.md) and
[native qualification contract](../packaging/linux/native-qualification.md).
Experimental targets must not be advertised as qualified desktop parity.

Installed-platform qualification includes native clipboard and protected content,
file imports and transfers above 4 MiB with exact hashes, cancellation and partial
cleanup, backup/restore, pairing and revocation, desktop keyboard/window behavior,
and owned cache cleanup under interrupted operations. Physical Android evidence
cannot be replaced by emulator tests; portable tests cannot establish installed
Windows or macOS behavior.

Resource evidence must account for the application, daemon, and workers separately
and together. Unresolved budgets or missing native measurements cannot be called
qualified; resource qualification is not a publication gate. Never invent passing
thresholds or raise budgets to conceal regressions.

Recovery uses an exact-source qualified run and verifies the original signatures,
receipts, and digests before publishing. It reuses the qualified product bytes
without rebuilding them. See [development](development.md) for local validation.
