# ADR-0032: Bound source application icon identity metadata

Status: accepted, 2026-09-27.

## Decision

CopyPaste stores an optional normalized PNG icon beside the existing source
application bundle ID and display name. The PNG is limited to 32 KiB and a
decoded edge of at most 128px. Its JSON envelope is bounded to 48 KiB and is
included in the local storage quota and bounded history-page budget.

File metadata keeps its legacy flat `filename` and `mime_type` JSON spelling.
New envelopes can carry both that file metadata and an icon. Icon-bearing rows
require current peers; current clients still read legacy file-only rows.

The existing encrypted local database protects the stored metadata. P2P carries
it inside Noise, while cloud signatures bind it to the row. Cloud metadata is
not content-encrypted, and this decision adds no Supabase column or server
migration.

## Validation

Core tests cover decoder dimensions, compressed and base64 bounds, strict
envelopes, legacy file parsing, sensitive-item icon stripping, storage quota,
bounded pages, P2P metadata shape validation, and cloud signature binding.
