# Architecture Decision Records (ADRs)

This directory contains the Architecture Decision Records for CopyPaste. ADRs
capture significant architectural and design decisions, the context in which
they were made, and their consequences. The format follows Michael Nygard's
original ADR template (see [`ADR-TEMPLATE.md`](./ADR-TEMPLATE.md)).

## Index

| ADR | Title | Status |
|-----|-------|--------|
| [001](./001-xchacha20-not-aesgcm.md) | XChaCha20-Poly1305 over AES-GCM for clipboard encryption | Accepted |
| [002](./002-unix-socket-ipc.md) | Unix socket IPC | Accepted |
| [003](./003-sqlcipher-at-rest.md) | SQLCipher at rest | Accepted |
| [004](./004-sqlite-wal.md) | SQLite WAL mode | Accepted |
| 005 | Slint as UI framework | Superseded by ADR-013 (file removed) |
| [013](./ADR-013-tauri-ui.md) | Tauri v2 + React as UI framework | Accepted |
| [007](./ADR-007-ipc-protocol-versioning.md) | IPC protocol versioning | Accepted |
| [008](./ADR-008-pake-protocol-choice.md) | PAKE protocol choice | Accepted |
| [009](./ADR-009-relay-storage-choice.md) | Relay storage choice | Accepted |
| [010](./ADR-010-codesigning-ad-hoc.md) | Code signing (ad-hoc) | Accepted |

> Note: legacy ADR-001..004 use the lowercase `NNN-slug.md` naming. New ADRs
> follow the `ADR-NNN-slug.md` convention described below. Renaming legacy
> files is tracked separately and intentionally not done in-place to preserve
> historical links.

## Naming Convention

- Filename: `ADR-NNN-kebab-case-title.md` (e.g. `ADR-011-relay-rate-limiting.md`).
- `NNN` is a zero-padded sequential integer starting at `001`.
- Numbers are **never reused**, even if an ADR is deprecated or superseded.
- The H1 line of each ADR matches `# ADR-NNN: <Title>`.

## Numbering Rules

1. The next ADR takes the highest existing number + 1. Gaps are allowed only
   when an ADR was withdrawn before merge; once merged, an ADR keeps its
   number forever.
2. Duplicate numbers are not permitted. The lint script
   (`scripts/check-adr-format.sh`) enforces this.
3. To replace an existing decision, write a new ADR and mark the old one
   `Superseded by [ADR-MMM](./ADR-MMM-slug.md)`. Do not edit the original
   decision text; add a "Status" note instead.

## Required Sections

Every ADR (excluding this README and the template) must contain:

- `# ADR-NNN: <Title>` — H1 header matching the filename number.
- `## Status` — one of `Proposed`, `Accepted`, `Deprecated`, or
  `Superseded by [ADR-MMM]`.
- `## Context` — what forced the decision.
- `## Decision` — what was decided.
- `## Consequences` — resulting tradeoffs.

`## Alternatives Considered` is recommended but optional.

## Lint

Run the format checker before opening a pull request:

```bash
scripts/check-adr-format.sh
```

Use `--fix` to scaffold missing sections, or `--help` for full usage.
