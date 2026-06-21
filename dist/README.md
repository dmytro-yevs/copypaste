# `dist/` — Release Artefacts

This directory holds **every shippable CopyPaste release artefact**. Nothing
else (no cargo intermediates, no test outputs, no per-arch staging binaries)
belongs here.

## Rules

1. **All release artefacts live in `dist/` only.** `target/` is for cargo
   intermediates and per-target binaries — never for shipped artefacts.
   Build scripts must output finished `.dmg` / `.apk` / `.zip` to `dist/`.
2. **One canonical filename per artefact.** No duplicates under different
   names; no version drift between filename and `Cargo.toml` version.
3. **Every artefact ships with a matching `.sha256`.** Generated via
   `shasum -a 256 <artefact> > <artefact>.sha256` from inside `dist/`.

## Naming convention

```
CopyPaste-v<full-version>-<platform>-<arch>.<ext>
CopyPaste-v<full-version>-<platform>-<arch>.<ext>.sha256
```

Where:

| Token            | Allowed values                                   |
|------------------|--------------------------------------------------|
| `<full-version>` | Matches the git tag exactly, e.g. `0.2.0-beta.1` |
| `<platform>`     | `macos`, `windows`, `android`, `linux`           |
| `<arch>`         | `arm64`, `x86_64`, `universal`                   |
| `<ext>`          | `dmg` (macOS), `zip` (Windows), `apk` (Android)  |

### Examples (current beta)

```
CopyPaste-v0.2.0-beta.1-macos-arm64.dmg
CopyPaste-v0.2.0-beta.1-macos-arm64.dmg.sha256
CopyPaste-v0.2.0-beta.1-macos-x86_64.dmg
CopyPaste-v0.2.0-beta.1-macos-x86_64.dmg.sha256
CopyPaste-v0.2.0-beta.1-windows-x86_64.zip
CopyPaste-v0.2.0-beta.1-windows-x86_64.zip.sha256
CopyPaste-v0.2.0-beta.1-android-arm64.apk
CopyPaste-v0.2.0-beta.1-android-arm64.apk.sha256
```

## Build scripts that write here

| Script                                  | Produces                                    |
|-----------------------------------------|---------------------------------------------|
| `scripts/release/_sign-and-dmg.sh`      | `CopyPaste-v<ver>-macos-<arch>.dmg` + sha   |
| `scripts/release/build-dmg-ci.sh`       | `CopyPaste-v<ver>-macos-<arch>.dmg` + sha   |
| `.github/workflows/release.yml`          | `CopyPaste-v<ver>-android-arm64.apk` + sha  |

Per-arch staging binaries (raw `copypaste-daemon`, `copypaste`, `*.so`)
go under `builds/<platform>-<arch>/`, not `dist/`. Those are inputs to the
artefact scripts above — they are **not** release artefacts themselves.

## Transient sub-directories

`dist/` may contain build-time staging directories that are inputs to the
final artefacts:

- `dist/CopyPaste.app/` — staged macOS bundle (consumed by `_sign-and-dmg.sh`)
- `dist/copypaste-windows-x86_64/` — staged Windows tree (zipped into the
  `.zip` artefact)

These stay in `dist/` for convenience but are not themselves the shipped
artefact — only the matching `*.dmg` / `*.zip` / `*.apk` is.

## Verifying

```sh
cd dist
shasum -a 256 --check CopyPaste-v0.2.0-beta.1-macos-arm64.dmg.sha256
```
