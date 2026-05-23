# Software Bill of Materials (SBOM)

CopyPaste publishes a **CycloneDX SBOM** for every signed release tag.
This page explains what that means, where to find the artifact, and how
to regenerate it locally.

## What is CycloneDX?

[CycloneDX](https://cyclonedx.org/) is an OWASP-backed, standards-based
SBOM format. A SBOM is a structured inventory of every third-party
dependency that ships inside the binary, including:

- crate name, version, license, source URL
- transitive (indirect) dependencies pulled in by Cargo
- per-component hashes and PURL identifiers

Auditors, downstream packagers (Homebrew, distro maintainers), and
security scanners (Grype, Trivy, Dependency-Track) consume CycloneDX
JSON/XML to flag known CVEs without rebuilding from source.

## Where to find the SBOM

For every tag matching `v*.*.*` the [`SBOM (CycloneDX)`](../../.github/workflows/sbom.yml)
workflow runs and uploads `sbom-<tag>.tar.gz`:

1. **GitHub Release page** — attached as a release asset alongside the
   DMG / archives. Direct download, no auth required.
2. **Workflow artifacts** — available for 90 days under
   *Actions → SBOM (CycloneDX) → run for tag*. Useful for unsigned
   workflow_dispatch runs.

The archive expands to:

```
sbom/
  <crate-name>/bom.json     # one per workspace member (copypaste-core, -daemon, -cli, ...)
  workspace.bom.json        # aggregated index pointing at every per-crate file
```

## Regenerating locally

```bash
# JSON (default)
bash scripts/gen-sbom.sh

# XML for tooling that prefers it
bash scripts/gen-sbom.sh --format xml

# Custom location
bash scripts/gen-sbom.sh --output-dir build/sbom

# Inspect without invoking cargo
bash scripts/gen-sbom.sh --dry-run
```

The script will install `cargo-cyclonedx` on demand (interactive prompt)
or you can pre-install it:

```bash
cargo install cargo-cyclonedx --locked
```

Output lands in `reports/sbom/` by default — `reports/` is gitignored,
so generated SBOMs never accidentally land in commits.

## Consuming the SBOM

Examples with common scanners:

```bash
# Grype (Anchore) — scan the JSON SBOM for CVEs
grype sbom:reports/sbom/copypaste-core/bom.json

# Trivy
trivy sbom reports/sbom/copypaste-daemon/bom.json

# Dependency-Track — upload via REST
curl -X POST -H "X-API-Key: $DT_KEY" \
  -F "bom=@reports/sbom/workspace.bom.json" \
  https://deptrack.example.org/api/v1/bom
```

## Related

- ADR on supply-chain hardening — see `docs/adr/` (forthcoming).
- Reproducible builds — see `scripts/build-in-docker.sh`.
- `cargo audit` runs weekly via [`.github/workflows/audit.yml`](../../.github/workflows/audit.yml).
