# Consolidated architecture and release audit

Date: 2026-08-26

Scope: release/evidence governance; History, Capture, and Onboarding; Rust/Tauri/TypeScript/Kotlin/native contracts; prior-product and compatibility surfaces

Subject: the current working tree at the audit baseline

## Method and governing decisions

This document consolidates three read-only audits. Equivalent findings are grouped under one owner and one migration path; the source-audit mapping at the end proves that no original finding was dropped. The audit did not edit production code or generated files and did not treat builds, caches, or generated artifacts as evidence.

Two repository decisions control every recommendation:

- The product opens one current database and must not probe, repair, migrate, or explain prior-product data (`AGENTS.md:49-52`; `docs/rewrite/target-architecture.md:198-202`).
- Port-manifest **behaviour, security, accessibility, and recovered defect tests remain binding**. Prior-product byte layouts, migration ladders, legacy verbs, wire compatibility, and old visual design are reference only (`docs/rewrite/port-manifest/README.md:8-39`). Removing legacy support must not remove the behaviour that the manifests preserve.

Severity means:

- **High**: publication can accept invalid evidence, current behaviour can corrupt or misrepresent user data, or a prohibited prior-product path is active.
- **Medium**: independently authored contracts have already diverged or a fail-open gate can be satisfied without the intended behaviour.
- **Low**: duplicate or dead surface whose present impact is bounded but which should be removed with its owning stream.

## Gate snapshot from the source audits

- `scripts/check-feature-ledger.py` passed without `--require-complete` while reporting 21 pending native states.
- `scripts/release/check-native-parity-wiring.py` passed despite the physical-Android contradiction in H1.
- `scripts/release/check-wiring.py --strict` failed because `copypaste-feedback` and `copypaste-retry` were absent from all Windows workspace shards.
- The comment baseline was not stale: all 103 entries remained over budget.

These results describe the audited working tree, not a fresh execution by this consolidation.

## Priority 0 — remove all prior-product and compatibility surface

### P0 (High) — prohibited compatibility paths are active

The repository says there is one schema and no prior-product migration path, yet every validated database open calls `upgrade_if_legacy_v2` (`crates/copypaste-core/src/storage/dbfile.rs:35-50`). That upgrader detects older layouts, rebuilds tables and indexes, supplies missing columns, copies rows, and drops a migration table (`crates/copypaste-core/src/storage/schema_upgrade.rs:1-54`, `:57-147`). This is an active compatibility ladder, not historical documentation.

The current IPC also retains prior pairing verbs and their implementations:

- `Method::PairCreate` and `Method::PairAccept` remain in the wire enum (`crates/copypaste-ipc/src/lib.rs:202-216`).
- The daemon routes both and keeps a refusal handler for the one-step accept path (`crates/copypaste-daemon/src/server/dispatch.rs:129-138`, `:179-190`; `crates/copypaste-daemon/src/p2p/handlers.rs:33-65`).
- The CLI still serializes `PairAccept`, and `Node::pair_create` remains a legacy alias (`crates/copypaste-cli/src/client.rs:1228`; `crates/copypaste-p2p/src/node/mod.rs:328-337`).

Source-compatibility residue also remains inside the current UI: `ClipKind`/`clipKind` are no-op aliases (`crates/copypaste-ui/src/features/history/model/clipKind.ts:1-7`), `InlineNotice.Icon` is a dead component-valued compatibility branch (`crates/copypaste-ui/src/components/shared/InlineNotice.tsx:8-46`), and `DefinitionList` duplicates the canonical `MetadataList` while active consumers remain (`docs/ui-architecture.md:232-239`; `crates/copypaste-ui/src/components/shared/DefinitionList.tsx:1-30`). These are not prior-product file readers, but the requested maximum cleanup includes them.

**Canonical result:** one current storage schema; one current IPC method set; one current component API. Remove `schema_upgrade`, its open hook and fixtures; remove the legacy pairing variants, CLI/daemon/node handlers, dispatch arms, tests and prose; migrate remaining `DefinitionList` consumers; remove no-op aliases and old-path exports. Remove dead `v2-main` filters and references to deleted `CLAUDE.md` rules while touching their owners.

Historical format sections in the port manifests may be deleted only after their still-binding behavioural/security assertions and defect IDs are moved into maintained specifications/tests. A broad text search is not a deletion plan: `@vitejs/plugin-legacy` supports old **WebView engines**, not the prior product, and `openai_legacy` is a live sensitive-key rule. Neither belongs to this purge unless the supported-platform contract changes.

**Fail-closed tests:**

- An existing file whose schema differs from the canonical schema is rejected unchanged; no repair/probe path runs.
- Repository search finds no `schema_upgrade`, `upgrade_if_legacy_v2`, `PairCreate`, `PairAccept`, `pair_create` alias, `ClipKind`, `clipKind`, `InlineNotice.Icon`, or old-path re-export in production.
- The one current database filename, schema, key path, AEAD path, and method inventory remain covered.
- Every behavioural/security/accessibility acceptance test retained from a historical manifest still passes after reference-format prose and fixtures are removed.

## High findings

### H1 — Android publication requires an emulator while policy requires hardware

The maintained policy requires same-commit macOS, **physical Android**, and installed Windows receipts (`docs/development.md:34-41`). The receipt schema instead fixes Android to `environment: "emulator"` (`crates/copypaste-ui/scripts/native-parity-evidence.schema.json:129-150`), and the consumer repeats the same requirement (`crates/copypaste-ui/scripts/native-parity-gate.mjs:10-33`). The publication gate downloads `release-android-smoke-evidence` from the emulator smoke job (`.github/workflows/release.yml:1067-1077`, `:1366-1388`). The wiring checker even asserts that no hardware job or dependency exists (`scripts/release/check-wiring.py:751-761`). The producer can already report a physical device (`scripts/release/android-smoke-release.sh:318-328`), but the consumer rejects it.

**Canonical owner:** a native-evidence policy consumed by the schema, writer, validator, and workflow. Publication accepts only a physical-device Android receipt. Emulator/API-33 jobs remain separate compatibility/nightly gates and must explicitly reject physical-device receipts when validating the inverse policy.

**Tests:** publication rejects emulator Android evidence and accepts physical-device evidence; nightly/emulator policy does the inverse; all receipts bind the same commit and run.

### H2 — feature completion and evidence provenance are fail-open

Three defects combine into one false-claim path:

1. A feature may be marked `removed`, continue owning shipped Tauri commands, and bypass platform validation because contracts are classified before the `product`-only native check (`scripts/check-feature-ledger.py:526-565`).
2. Android/macOS completion accepts arbitrary non-empty `evidence_states` strings plus representative screenshot/accessibility paths (`scripts/feature_ledger_evidence.py:201-220`, `:250-281`). The receipt schema carries no feature or state identity (`crates/copypaste-ui/scripts/native-parity-evidence.schema.json:6-100`). A ledger edit can therefore claim verified states without a same-run receipt for each state.
3. Execution provenance is inferred from raw substrings. Script closure follows path-like strings in comments (`scripts/feature_ledger_evidence.py:63-79`), `executed_by` accepts a pathname anywhere in a file (`scripts/check-feature-ledger.py:176-196`), output production is regex/substr-based (`scripts/feature_ledger_evidence.py:178-198`), and cloud-state coverage searches raw `capture_state ...` text (`scripts/release/check-wiring.py:983-995`). Commented-out calls can keep claims green.

Windows has a separate exact-state table, which is another owner rather than a solution.

**Canonical owner:** `docs/feature-ledger.schema.json` owns feature status and declared feature/platform/state IDs. Each runtime receipt names the feature ID and each state exactly once, and binds source commit/run/scenario. A `removed` feature owns no registered command, route, capability, workflow artifact, or native registration.

**Tests:** removed+shipped fails; missing/duplicate command ownership fails; verified states require exact same-run receipts; missing, unexpected, duplicate, and unregistered states fail; comments and inert strings never count as execution or output.

### H3 — Android binary paste-back writes display placeholders as user data

Android decrypts binary rows and converts them to labels such as `[image]`, `[file]`, or `[unsupported]` (`crates/copypaste-ui/src-tauri/src/backend/embedded/rows.rs:37-58`). `EmbeddedBackend::copy` then writes that display label as ordinary text, and `copy_as_plain_text` aliases the same operation (`crates/copypaste-ui/src-tauri/src/backend/embedded/items.rs:178-191`; `crates/copypaste-ui/src-tauri/src/backend/embedded/mod.rs:66-72`). Desktop dispatches authenticated bytes by content class (`crates/copypaste-daemon/src/server/items.rs:149-179`); Windows writes images but refuses files (`crates/copypaste-daemon/src/clipboard/windows.rs:324-355`). Desktop `copy_plain_text` also aliases native copy (`crates/copypaste-daemon/src/server/items.rs:182-190`).

**Canonical owner:** a shared `ClipboardPayload::{Text, Image, File}` plus an explicit textual-representation operation. Unsupported native writes return a typed error; a display label is never written as content. Until Android binary writing exists, it must fail closed.

**Tests:** payload conformance for text/image/file/unknown; Android proves binary never becomes placeholder text; native macOS/Windows paste tests; explicit plain-text behaviour for every class.

### H4 — pairing deadlines and terminal states diverge across native targets

Rust owns a 60-second confirmation timeout (`crates/copypaste-p2p/src/node/pairing.rs:11-30`), and AT-29 requires the SAS and decision buttons to disappear at that deadline (`docs/rewrite/port-manifest/06-ui-behaviour.md:1368-1379`). Windows repeats 60 seconds locally (`crates/copypaste-ui/src-tauri/src/pairing_presentation/windows/confirm.rs:1-15`, `:116-135`). Android repeats `60_000`, sends `cancel`, then paints `timed_out` (`crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/PairingDialogController.kt:21-34`, `:145-167`). macOS presents a blocking confirmation alert without a watchdog (`crates/copypaste-ui/src-tauri/src/pairing_presentation/macos.rs:159-173`). Native copy for the same state also differs.

**Canonical owner:** the Rust pairing ceremony exposes an authoritative deadline and UI-safe semantic state. Native surfaces protect/render/dismiss and re-read progress; they do not invent transitions or terminal copy.

**Tests:** controlled-clock invite/SAS expiry on Android, macOS, and Windows; timeout never becomes cancel; close/abort is idempotent; SAS digits and decisions disappear; QR/SAS stay out of the WebView and accessible native semantics remain intact.

## Medium findings

### M1 — the Android security manifest guard is fail-open and text-based

A missing manifest exits successfully as `SKIP`; permissions, queries, and services are checked with `grep`, so XML comments can satisfy positive checks (`scripts/check-android-manifest.sh:10-47`, `:49-70`). The repository already parses the manifest with an XML DOM (`scripts/release/product-config.mjs:88-104`).

**Canonical owner:** one structured Android-manifest module reused by generators and guards. Missing/malformed targets fail. Tests cover a missing manifest and comments containing every required/forbidden string.

### M2 — local CI claims parity while running different gates

The local WSL entry point says it mirrors CI but runs the advisory file-size checker (`scripts/prepush/wsl/verify.sh:1-19`), whose porcelain mode always exits zero (`scripts/check-file-size.sh:60-70`). CI runs the enforcing gate (`.github/workflows/ci.yml:190-205`). The local release entry runs only the feature-ledger self-test, while CI runs the real validation (`scripts/release/check.sh:400-413`; `.github/workflows/ci.yml:59-70`). The Windows shard checker correctly detects that workspace members `copypaste-feedback` and `copypaste-retry` (`Cargo.toml:3-17`) appear in none of the shard commands (`.github/workflows/ci.yml:728-732`, `:761-766`, `:795-803`).

**Canonical owner:** one machine-readable gate registry or executable dispatcher consumed by local and CI entry points. Workspace shards derive from `cargo metadata` and cover every package exactly once.

**Tests:** local/CI gate manifests compare equal; an oversized fixture fails both; shard coverage equals metadata exactly once; no self-test substitutes for the real gate.

### M3 — platform, toolchain, artifact, and port policy has too many owners

The same semantic values are independently repeated across producer scripts, JSON Schema, validators, Gradle, Cargo, workflows, and checkers:

- Native platform/environment/scenario/assertion/budget/artifact policy appears in the writer, schema, and JS gate (`scripts/release/write-native-evidence.py:14-34`; `crates/copypaste-ui/scripts/native-parity-evidence.schema.json:18-99`; `crates/copypaste-ui/scripts/native-parity-gate.mjs:10-51`).
- Android API sets, SDK/NDK/build-tools/Java pins and four ABI triples are repeated in workflow inputs/matrices and shell helpers (`.github/workflows/android-emulator.yml:65-77`, `:135-170`, `:401-409`; `scripts/release/android-link-abis.sh:19-32`; `scripts/release/android-ndk-env.sh:27-32`).
- Cargo declares Rust 1.96 while `rust-toolchain.toml` floats `stable`; CI/release repeat 1.96 in commands, actions and cache keys (`Cargo.toml:19-23`; `rust-toolchain.toml:1-11`; `.github/workflows/ci.yml:35-57`).
- Release artifact IDs, filenames, architecture maps, dev/cloud ports and obsolete `v2-main` filters are reconstructed at multiple call sites. Android push and pull-request paths are duplicated without a structural equality test (`.github/workflows/android-emulator.yml:82-95`).

**Canonical owners:** `config/platform-matrix.json`, `config/native-evidence-policy.json`, and `config/release-artifacts.json`; Cargo workspace metadata for Rust/product version/application IDs; `.nvmrc` for Node major; Tauri `devUrl` for the UI port. Generated projections get stale-output tests. Job runner, timeout, retention, concrete upload directory, and the named coverage set selected by a job remain workflow-local.

### M4 — commands, events, protocol, errors, and updater states are not one generated contract

Rust owns the runtime command list, but TypeScript accepts any command string and 75 distinct wrappers spell literals independently (`crates/copypaste-ui/src/lib/ipcCall.ts:223-227`; `crates/copypaste-ui/src-tauri/src/lib.rs:239-329`). The dev bridge and feature ledger add more inventories. Rust `ChangePayload` includes `swept`, while TypeScript omits it and Diagnostics repairs the type with an intersection (`crates/copypaste-ui/src-tauri/src/service/push.rs:50-67`; `crates/copypaste-ui/src/hooks/usePush.ts:35-44`; `crates/copypaste-ui/src/hooks/useDiagnostics.ts:18-20`). Protocol version and boundary/updater error codes are also copied into TypeScript; Android updater results remain raw strings (`crates/copypaste-ipc/src/lib.rs:53-69`; `crates/copypaste-ui/src/lib/errors.ts:10-54`; `crates/copypaste-ui/src-tauri/src/updater/android.rs:1-75`).

**Canonical owner:** Rust-owned Tauri command, event payload, protocol metadata, UI-boundary error, and updater contracts generating TypeScript runtime values/client signatures and Kotlin bridge values. Unknown future daemon codes remain preservable.

**Tests:** every registered handler appears exactly once; unknown commands fail typecheck; event serialization includes `swept`; protocol has one authoring location; known errors/retryability are exhaustive; Kotlin/TS generated output is current.

### M5 — content, device, platform, and backend capabilities are guessed independently

Rust classifies unknown `application/*` as `Other`; TypeScript usually calls it a file (`crates/copypaste-ipc/src/content_type.rs:31-60`; `crates/copypaste-ui/src/lib/format.ts:88-127`). Raw `Item.content_type` remains an open string with no generated base class (`crates/copypaste-ui/src/generated/ipc.ts:83-87`). Device platform/class enums exist in both P2P and IPC, real origins are recorded as unknown, and React guesses device kind from display text (`crates/copypaste-p2p/src/device_profile.rs:9-79`; `crates/copypaste-ipc/src/payload/device.rs:27-49`; `crates/copypaste-ui/src/components/shared/DeviceMeta.tsx:5-13`). The UI architecture explicitly forbids that guess (`docs/ui-architecture.md:208-210`; `docs/rewrite/port-manifest/06-ui-behaviour.md:420-422`). `AppPlatform` separately restates generated `PermissionHost` (`crates/copypaste-ui/src/lib/platform.ts:4-38`). A diagnostic clipboard-backend string is interpreted with a regex, and Windows names itself to satisfy it (`crates/copypaste-daemon/src/clipboard/windows.rs:357-362`).

**Canonical owner:** dependency-light Rust device/capability contracts and a generated `ContentClass` beside the open raw content type. Backend status exposes typed capability fields. Unknown stays unknown; no display-name or diagnostic-name promotion.

### M6 — permission and capture policy/presentation is split across Rust, Kotlin, and JSX

Generated permission statuses exist, but Android onboarding interprets them with partial string comparisons: `not_required` is accepted after a request but rendered as actionable later (`crates/copypaste-ui/src/generated/ipc.ts:31-39`; `crates/copypaste-ui/src/features/onboarding/patterns/AndroidCaptureSetup.tsx:22-65`). Kotlin duplicates notification and tile result policy (`crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/OnboardingPermissionGate.kt:1-21`; `crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/TileAddGate.kt:1-10`) even though Rust owns it (`crates/copypaste-ui/src-tauri/src/shell/permissions/policy.rs:27-52`). Kotlin also persists fallback user-facing capture wording despite ADR-0005 assigning wording to Rust (`docs/adr/0005-android-capture-in-rust-kotlin-reports.md:11-34`; `crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/CapturePlugin.kt:228-233`; `crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/CaptureService.kt:129-137`). Capture severity is centralized, but ARIA urgency differs: setup uses `alert` for faults while the History strip always uses `status` (`crates/copypaste-ui/src/features/capture/model/capturePresentation.ts:8-29`; `crates/copypaste-ui/src/features/capture/patterns/CaptureSetup.tsx:132-143`; `crates/copypaste-ui/src/features/capture/patterns/CaptureStatus.tsx:23-39`).

**Canonical owner:** Rust returns typed facts/results and finished wording; React-free exhaustive permission and capture presentation records own label/action/disabled/tone/ARIA role. Kotlin enforces only platform mechanics and the pre-read exclusion projection required by ADR-0005.

### M7 — History body and presentation state machines already disagree

`useItemBody` reports a full-body failure (`crates/copypaste-ui/src/hooks/useItemBody.ts:6-40`). The expanded reader renders an explicit unavailable state for `truncated + failed` (`crates/copypaste-ui/src/features/history/patterns/ClipDetailDialog.tsx:77-99`, `:207-210`), but the Inspector accepts `fullContentFailed` and never reads it, then falls back to the truncated list preview (`crates/copypaste-ui/src/features/history/patterns/LibraryInspectorPanel.tsx:25-52`, `:83-94`). This violates the architecture rule that a preview fragment is never presented as complete content (`docs/ui-architecture.md:128-143`).

The same surfaces also disagree on singular/filter labels and image copy label/icon (`crates/copypaste-ui/src/features/history/patterns/ClipDetailDialog.tsx:102-107`, `:298-308`; `crates/copypaste-ui/src/features/history/patterns/LibraryInspectorPanel.tsx:112-121`), and duplicate filename, origin, and absolute-time formatting (`crates/copypaste-ui/src/features/history/components/ClipCardBody.tsx:11-14`; `crates/copypaste-ui/src/features/history/components/InspectorPreview.tsx:10-13`; `crates/copypaste-ui/src/features/history/components/SourceMeta.tsx:48-65`).

**Canonical owner:** a React-free discriminated `clipBodyPresentation` resolver and one exhaustive History presentation record. Sensitive reveal plaintext stays ephemeral and outside query caches.

**Tests:** both surfaces show unavailable on full-body failure; reveal remains masked until explicit action and expires; image actions match; single-item labels are singular; filter labels remain intentional; Unix/Windows/empty filename, origin fallback, and time formatting have one test table.

### M8 — canonical UI primitives exist while features recreate them

History draws raw preview `Surface` compositions rather than `PreviewSurface` and still uses `DefinitionList` instead of `MetadataList` (`crates/copypaste-ui/src/features/history/patterns/LibraryInspectorPanel.tsx:1-11`, `:149-232`; `crates/copypaste-ui/src/components/shared/PreviewSurface.tsx:1-17`; `crates/copypaste-ui/src/components/shared/MetadataList.tsx:1-53`). Capture and Devices independently implement the planned `StatusCard`. `AppIcon` and `ClipImage` duplicate PNG base64 decoding and object-URL lifecycle (`crates/copypaste-ui/src/components/shared/AppIcon.tsx:8-25`; `crates/copypaste-ui/src/features/history/components/ClipImage.tsx:17-57`). Dead `ClipKind` and `InlineNotice.Icon` remain.

**Canonical owner:** existing `PreviewSurface`, `MetadataList`, a finite shared `StatusCard`, and one `usePngObjectUrl` result type. Migrate all consumers, then delete compatibility surfaces.

### M9 — security containment, redaction, and exclusion normalization are duplicated

Repository/artifact containment is implemented separately in Python and JavaScript. Desktop and Android E2E leak detectors repeat incomplete path regexes and miss UNC plus many absolute POSIX roots (`e2e/src/harness/leaks.ts:56-69`; `e2e-android/src/harness/leaks.ts:47-57`). Android evidence redaction hardcodes one AWS key shape rather than consuming shared sensitive-rule vectors (`e2e-android/src/harness/redact.ts:13-23`). Windows exclusion normalization is implemented independently in TypeScript and Rust; Android repeats normalization in Kotlin and the Rust write boundary (`crates/copypaste-ui/src/lib/exclusions.ts:17-47`; `crates/copypaste-daemon/src/clipboard/windows_attribution.rs:152-180`; `crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/CaptureExclusions.kt:11-40`; `crates/copypaste-ui/src-tauri/src/backend/embedded/items.rs:99-127`). The two enforcement boundaries are required; the normalization rules need not differ.

**Canonical owner:** one cross-language JSON fixture corpus, one helper per language, and separate producer/consumer containment enforcement. Cover traversal, absolute POSIX, drive, UNC, case, symlink parent/leaf, hardlink alias, and duplicate resolved-file cases.

### M10 — workflow validators duplicate parsers and execution graphs

Three large programs parse `release.yml`, artifact names, jobs, and publication dependencies independently. They use regex/string extraction for Cargo workspaces, semver, scripts and execution while structured parsers or installed packages already exist. Examples include raw workspace parsing instead of `cargo metadata`, a partial Node semver parser despite the UI `semver` dependency, and raw script/source closure (`scripts/release/check-wiring.py:39-80`, `:247-320`; `scripts/feature_ledger_evidence.py:63-79`).

**Canonical owner:** one structured workflow/artifact graph library. Use YAML AST/job-step semantics, `cargo metadata`, the maintained semver package, TOML/XML parsers, and explicit producer/consumer nodes. Comments never become edges. Split by workflow parsing, product policy, and validation orchestration rather than leaving another catch-all checker.

### M11 — Windows source-app discovery duplicates installed dependencies and FFI

Source icon resolution hand-writes App Paths registry calls while installed-app discovery already uses `winreg`; icon extraction directly wraps APIs covered by the existing `winsafe` dependency (`crates/copypaste-ui/src-tauri/src/source_app_icon/registry.rs:1-20`; `crates/copypaste-ui/src-tauri/src/installed_source_apps/windows.rs:1-29`; `crates/copypaste-ui/src-tauri/src/source_app_icon/win_icon.rs:7-25`). That expands the two direct Windows FFI boundaries accepted by ADR-0018 (`docs/adr/0018-keep-narrow-windows-ffi-boundaries.md:5-20`).

**Canonical owner:** one Windows catalogue/resolver using maintained dependencies; icon extraction consumes its result. Any unavoidable new FFI needs the repository's dependency exemption and ADR update.

### M12 — deep-link capability is registered but has no dispatcher

Cargo metadata, the Tauri plugin/capability/config, and Android manifest register `copypaste://pair`, but source search found no URL listener or pairing dispatcher (`Cargo.toml:26-31`; `crates/copypaste-ui/src-tauri/src/lib.rs:92`; `crates/copypaste-ui/src-tauri/capabilities/default.json:13`; `crates/copypaste-ui/src-tauri/tauri.conf.json:52`). This is either a half-shipped feature or dead capability surface.

**Canonical result:** implement one protected dispatcher on macOS, Android, and Windows with ledger states and native evidence, or remove the scheme, plugin, capability, manifest filters, metadata and tests together. Given the requested maximum cleanup, removal is the default unless the feature is explicitly retained.

## Low cleanup folded into owning streams

- Remove dead `v2-main` push filters; the audited remote had no such branch.
- Structurally compare Android push and pull-request path filters.
- Generate or share the macOS `arm64/x86_64 -> Rust triple` map instead of copying it across four scripts.
- Remove the mutation-manifest entry that knowingly invokes the ignored `check-docs.py --self-test` (`scripts/mutation-gate/mutations.json:39-46`).
- Replace active references to deleted `CLAUDE.md` rules, including the comment gate and leak helpers (`scripts/check-comments.sh:171-179`; `e2e/src/harness/leaks.ts:56-59`).
- Keep shrinking the 103-entry comment baseline; none is currently obsolete.

## Canonical source-of-truth matrix

| Semantic contract | Canonical owner | Generated/consuming direction |
|---|---|---|
| Current storage schema and open policy | `copypaste-core::storage::schema` | fresh create / exact verify only; no upgrader |
| Daemon and Tauri methods, payloads, events, protocol, known UI errors | Rust typed registries | Rust -> TS runtime/client bindings and Kotlin bridge values |
| Open content type plus base content/paste capability | `copypaste-ipc` Rust contract | Rust -> daemon, Tauri, TS, native writers |
| Device platform/class and backend capability | dependency-light Rust contract | P2P/IPC/Tauri -> TS; unknown remains unknown |
| Pairing deadline and semantic state | `copypaste-p2p` ceremony | IPC presentation DTO -> native renderers |
| Permission/capture policy and wording | Rust shell/capture model | Kotlin facts -> Rust decisions -> TS presentation |
| Feature status, state inventory, command ownership | feature-ledger JSON Schema + ledger | ledger -> validator and receipts |
| Native evidence environment/scenario/assertion/budget policy | `config/native-evidence-policy.json` | generate/validate schema, writer, gate, workflow projections |
| Platforms, Android APIs/SDK/NDK/Java/ABI mappings | `config/platform-matrix.json` | Gradle/scripts/workflows generated or checked projections |
| Release artifact IDs and filename contracts | `config/release-artifacts.json` | workflows, packagers, ledger and validators |
| Rust/product version and application identifiers | Cargo workspace package/metadata | CI/release/Gradle/Tauri projections |
| Node major | `.nvmrc` | every package engine range must admit exactly that major |
| CI/local gate inventory | one executable gate registry | CI and pre-push choose platform spellings behind the same gate IDs |
| History body/type/action presentation | React-free History model | Inspector, detail, rows and filters |
| Shared preview/metadata/status/media lifecycle | `components/shared` | feature composition only |
| Containment/redaction/exclusion cases | cross-language JSON vectors | one helper per language at independent trust boundaries |

## Required cross-cutting tests and evidence

1. **Legacy absence:** exact-schema open, rejected mismatched schema, one current method inventory, no old database/method/component symbols, no historical fixture read path.
2. **Release truth:** removed features own nothing; every verified feature/platform/state has one same-run receipt; physical Android is mandatory for publication; inert strings/comments do not count.
3. **Clipboard:** text/image/file/unknown conformance; no placeholder paste; plain-text operation specified separately; real macOS/Windows and Android instrumentation evidence.
4. **Pairing:** all states through one table; controlled-clock invite/SAS expiry; timeout != cancel; idempotent abort; protected, accessible native QR/SAS surfaces.
5. **Generated contracts:** exact command registration, event payload round-trips, protocol/error/updater exhaustiveness, generated TS/Kotlin stale-output checks.
6. **Platform projections:** Rust, Node, SDK/NDK/build-tools/Java/API/ABI/artifact generated projections fail when stale; workspace shards equal `cargo metadata` exactly once.
7. **Security:** manifest XML comments cannot satisfy checks; containment corpus covers traversal/absolute/UNC/links/case; secrets and paths remain absent from DOM, accessibility trees, logs and receipts.
8. **UI/accessibility:** History unavailable/masked states agree; exhaustive permissions; explicit alert/status policy; semantic `dl/dt/dd`; one scroll container; contain-only images; axe plus keyboard/focus checks.
9. **Native evidence:** Android hardware/emulator policies, macOS AX logs, Windows UIA, screenshots, measured latency, restart/offline/failure/timeout/unavailable states on the same commit.

## Migration DAG

```text
0. Add negative characterization tests and freeze the current base
   |
   +--> 1A. Remove prior-product storage/IPC/UI compatibility surface
   |
   +--> 1B. Add ledger schema, platform matrix, evidence policy,
   |        artifact manifest and CI gate registry (additive first)
   |
   +--> 1C. Add Rust command/event/error/content/device/pairing contracts
            and binding generation (additive first)
              |                         |
              v                         v
2A. Bind receipts to feature/state   2B. Switch TS/Kotlin/native consumers
    and require physical Android         to generated contracts
              |                         |
              +------------+------------+
                           v
3. Fix clipboard, pairing, permissions, History and security behaviour
   on the canonical contracts
                           |
                           v
4. Make local/CI use the same registry; generate/check every projection
                           |
                           v
5. Delete duplicate literals, regex parsers, overlapping validators,
   historical-only fixtures/prose and temporary adapters
                           |
                           v
6. Run one full cross-platform evidence wave, then mark ledger states complete
```

No existing ledger record becomes verified before step 2A. Emulator/API-33 evidence remains a pre-publication compatibility gate after the physical-device publication job exists.

## Parallel implementation streams

Use ordinary isolated Git worktrees and branches; do not use Orca. The initial wave can run at least twelve agents concurrently because ownership is split below. Shared workflow/generated-output integration is serialized by one integrator after focused review.

| Stream | Exclusive primary paths / responsibility | Depends on | Deliverable |
|---|---|---|---|
| S1 Legacy storage purge | `copypaste-core/src/storage/{dbfile,schema_upgrade,schema_verify,...}` | negative tests | remove upgrader and all old-schema fixtures; exact reject-only open |
| S2 Legacy IPC/CLI purge | `copypaste-ipc`, CLI client, daemon dispatch/peer handlers, node alias | negative tests | remove old pairing verbs and all reachability |
| S3 UI compatibility purge | `components/shared` legacy APIs, History model barrel, remaining consumers | none | migrate `DefinitionList`; remove `ClipKind`, old icon prop and re-exports |
| S4 Receipt and ledger truth | ledger schema/checker, evidence schema/writer/gate | policy files additive | feature/state receipts; removed owns nothing; physical environment policy |
| S5 Physical Android publication | Android hardware scenario and release job | S4 | same-commit hardware receipt; emulator stays separate |
| S6 Android manifest security | manifest checker and structured XML helper/tests | none | missing/malformed/comment fixtures fail closed |
| S7 CI/local gate parity | gate registry, pre-push scripts, CI shard projection | additive registry | identical gate sets; exact metadata shard coverage |
| S8 Rust/TS/Kotlin boundary generation | command/event/protocol/error/updater registries and generator | none | typed runtime values and client/bridge bindings |
| S9 Clipboard payload correctness | IPC content payload, daemon clipboard, embedded backend | contract design | no placeholder paste; explicit plain-text semantics |
| S10 Pairing semantics | P2P ceremony and three native presentation implementations | S2 for shared pairing files | authoritative deadline/state and protected expiry UI |
| S11 Device/permission/capture contracts | device contract, permission/capture Rust/Kotlin/TS models | generator seam from S8 | no string guesses or duplicated policy/wording |
| S12 History presentation | History hooks/model/Inspector/detail/row formatters | generated content class from S11 if adopted | one body resolver and action/label/formatter table |
| S13 Shared UI/media primitives | `PreviewSurface`, `MetadataList`, new `StatusCard`, PNG hook | coordinate final deletions with S3 | canonical preview/metadata/status/media lifecycle |
| S14 Security vector corpus | E2E leak/redaction helpers, exclusion normalization tests/helpers | none | shared vectors with per-language helpers and two trust boundaries |
| S15 Platform/artifact projections | new config manifests, Gradle/Cargo/workflow projection checks | none; workflow edits integrate after S4/S7 | one owner for SDK/API/ABI/toolchain/artifact values |
| S16 Windows catalogue/FFI | installed apps and source icon modules | none | one `winreg`/`winsafe` resolver; ADR compliance |
| S17 Deep-link removal/feature decision | Cargo/Tauri capability/config/native registrations | product decision; coordinate Android manifest with S6 | complete cross-platform dispatcher or total removal |

Safe initial concurrent set: S1, S3, S4, S6, S7, S8, S9, S11, S12, S14, S15, S16 and S17. Defer S5 until S4 defines the receipt; defer S10's overlapping pairing files until S2 lands; coordinate S13's deletion pass with S3. `.github/workflows/release.yml`, `.github/workflows/ci.yml`, generated bindings, and shared policy manifests have one named owner during integration even when upstream work was parallel.

Each stream reports base SHA, final commit, exact paths, focused tests, acceptance criteria, unverified platforms, risks, and discovered independent work. A completion report is not integration approval.

## Source-audit mapping

Labels below refer to the three input audits: **R** release/evidence governance, **U** History/Capture/Onboarding architecture, and **X** cross-language/native contracts.

| Consolidated finding | Source findings absorbed |
|---|---|
| P0 prohibited compatibility paths | requested top priority; U-L1/U-L7/U-L8; active storage/IPC evidence added from primary source |
| H1 physical Android inverted gate | R-H1 |
| H2 fail-open feature/evidence provenance | R-H2, R-H3, R-H4 |
| H3 binary paste corruption | X-H1; U-M4 and X-M2 feed its content-class owner |
| H4 pairing deadline/state divergence | X-H2 |
| M1 Android manifest guard | R-H5 |
| M2 CI/local mismatch and shards | R-H6, R-M1; mutation self-test low finding |
| M3 platform/toolchain/artifact/port owners | R-M2, R-M3, R-M4, R-M5, R-M6, R-M10; R low branch/filter/arch/port findings; X-L3 |
| M4 generated boundary contracts | U-M2, U-M3, U-M8, U-M9; X-M1, X-M5, X-L1, X-L2 |
| M5 typed content/device/platform/backend capability | U-M4, U-M5, U-M10; X-M2, X-M3, X-M4 |
| M6 permission/capture policy and presentation | U-M6, U-M7; X-M6, X-M10 |
| M7 History body/presentation | U-M1, U-M11, U-M12, U-L3, U-L5, U-L6 |
| M8 shared UI primitives/media | U-M13, U-L1, U-L2, U-L4, U-L7, U-L8 |
| M9 security corpus/exclusions | R-M9; X-M7 |
| M10 structured workflow validation | R-M7, R-M8 and the execution-graph part of R-H4 |
| M11 Windows catalogue/FFI | X-M8 |
| M12 deep-link capability | X-M9 |

No generated file is an authoring target. Change the upstream Rust/schema/config owner and regenerate `crates/copypaste-ui/src/generated/ipc.ts`, Tauri schemas and generated Kotlin values through their maintained generators.

## Final assessment

The immediate release blockers are the active compatibility paths, the inverted physical-Android gate, fail-open feature/evidence completion, binary paste corruption, and divergent pairing timeouts. The architectural debt beneath them has one repeated cause: production semantics are authored independently in Rust, TypeScript, Kotlin, JSON Schema, shell and workflow YAML.

The migration must therefore remove prior-product surface first, add negative tests before changing contracts, establish typed/machine-readable owners, switch consumers one boundary at a time, and delete mirrors only after a full cross-platform evidence wave is green. UI cleanup follows the same rule at a smaller scale: one body resolver, one presentation map, one primitive family, no compatibility aliases after the final consumer moves.
