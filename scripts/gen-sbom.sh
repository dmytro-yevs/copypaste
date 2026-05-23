#!/usr/bin/env bash
# gen-sbom.sh — generate CycloneDX Software Bill of Materials (SBOM)
# for every workspace crate via `cargo cyclonedx`, plus an aggregated
# workspace.bom.<ext> for release-bundle consumption.
#
# Examples:
#   scripts/gen-sbom.sh                                   # JSON SBOMs under reports/sbom/
#   scripts/gen-sbom.sh --format xml                      # XML SBOMs
#   scripts/gen-sbom.sh --output-dir build/sbom           # custom output directory
#   scripts/gen-sbom.sh --dry-run                         # show what would run
#
# Output layout (default):
#   reports/sbom/<crate>/bom.json    # per-crate SBOM (one per workspace member)
#   reports/sbom/workspace.bom.json  # aggregated workspace SBOM
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

FORMAT="json"
OUTPUT_DIR="reports/sbom"
DRY_RUN=0

usage() {
    cat <<EOF
${SCRIPT_NAME} — generate CycloneDX SBOM for the Cargo workspace.

Usage: ${SCRIPT_NAME} [options]

Options:
  --format <json|xml>     SBOM serialization format (default: json).
  --output-dir <path>     Output directory (default: reports/sbom).
  --dry-run               Print actions without invoking cargo.
  -h, --help              Show this help and exit.

Notes:
  Requires cargo-cyclonedx. If missing, the script will prompt to install
  it via 'cargo install cargo-cyclonedx --locked'.
EOF
}

log()  { printf '[%s] %s\n' "${SCRIPT_NAME}" "$*"; }
die()  { printf '[%s] error: %s\n' "${SCRIPT_NAME}" "$*" >&2; exit 1; }

while [[ $# -gt 0 ]]; do
    case "$1" in
        --format)
            [[ $# -ge 2 ]] || die "--format requires a value"
            FORMAT="$2"
            shift 2
            ;;
        --output-dir)
            [[ $# -ge 2 ]] || die "--output-dir requires a value"
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            die "unknown argument: $1 (use --help)"
            ;;
    esac
done

case "${FORMAT}" in
    json|xml) ;;
    *) die "invalid --format '${FORMAT}' (expected: json or xml)" ;;
esac

# Resolve output dir relative to repo root if not absolute.
if [[ "${OUTPUT_DIR}" != /* ]]; then
    OUTPUT_DIR="${ROOT_DIR}/${OUTPUT_DIR}"
fi

ensure_cyclonedx() {
    if command -v cargo-cyclonedx >/dev/null 2>&1; then
        return 0
    fi
    if cargo cyclonedx --version >/dev/null 2>&1; then
        return 0
    fi

    log "cargo-cyclonedx not found."
    if [[ ${DRY_RUN} -eq 1 ]]; then
        log "[dry-run] would install: cargo install cargo-cyclonedx --locked"
        return 0
    fi

    if [[ ! -t 0 ]]; then
        die "cargo-cyclonedx missing and stdin is not a TTY (install manually: cargo install cargo-cyclonedx --locked)"
    fi

    printf 'Install cargo-cyclonedx now via cargo install? [y/N] '
    read -r reply
    case "${reply}" in
        y|Y|yes|YES)
            log "installing cargo-cyclonedx ..."
            cargo install cargo-cyclonedx --locked
            ;;
        *)
            die "cargo-cyclonedx is required; aborting"
            ;;
    esac
}

main() {
    cd "${ROOT_DIR}"
    ensure_cyclonedx

    log "format        : ${FORMAT}"
    log "output dir    : ${OUTPUT_DIR}"
    log "workspace root: ${ROOT_DIR}"

    if [[ ${DRY_RUN} -eq 1 ]]; then
        log "[dry-run] mkdir -p ${OUTPUT_DIR}"
        log "[dry-run] cargo cyclonedx --format ${FORMAT} --all --override-filename bom"
        log "[dry-run] would move per-crate bom.${FORMAT} into ${OUTPUT_DIR}/<crate>/"
        log "[dry-run] would emit aggregated workspace.bom.${FORMAT}"
        return 0
    fi

    mkdir -p "${OUTPUT_DIR}"

    # Generate per-crate SBOMs; cargo-cyclonedx writes bom.<ext> into each
    # crate's manifest directory.
    # Note: cargo-cyclonedx renamed --output-pattern to --override-filename
    # in v0.5.x. We pin/expect v0.5.7+; older 0.4.x is incompatible.
    cargo cyclonedx --format "${FORMAT}" --all --override-filename bom

    # Collect per-crate outputs into ${OUTPUT_DIR}/<crate>/bom.<ext>
    local crate_dir crate_name bom_path dest_dir
    while IFS= read -r crate_dir; do
        bom_path="${crate_dir}/bom.${FORMAT}"
        [[ -f "${bom_path}" ]] || continue
        crate_name="$(basename "${crate_dir}")"
        dest_dir="${OUTPUT_DIR}/${crate_name}"
        mkdir -p "${dest_dir}"
        mv "${bom_path}" "${dest_dir}/bom.${FORMAT}"
        log "wrote ${dest_dir}/bom.${FORMAT}"
    done < <(find "${ROOT_DIR}/crates" -mindepth 1 -maxdepth 1 -type d 2>/dev/null)

    # Aggregated workspace SBOM (best-effort: concatenate component refs).
    # cargo-cyclonedx does not emit a single workspace bom, so we synthesise
    # a manifest pointing at every per-crate file.
    local agg="${OUTPUT_DIR}/workspace.bom.${FORMAT}"
    if [[ "${FORMAT}" == "json" ]]; then
        {
            printf '{\n'
            printf '  "bomFormat": "CycloneDX",\n'
            printf '  "specVersion": "1.5",\n'
            printf '  "metadata": { "tool": "gen-sbom.sh" },\n'
            printf '  "components": [],\n'
            printf '  "externalReferences": [\n'
            local first=1
            for f in "${OUTPUT_DIR}"/*/bom.json; do
                [[ -f "$f" ]] || continue
                [[ ${first} -eq 1 ]] || printf ',\n'
                first=0
                printf '    { "type": "bom", "url": "%s" }' "${f#${OUTPUT_DIR}/}"
            done
            printf '\n  ]\n}\n'
        } > "${agg}"
    else
        {
            printf '<?xml version="1.0" encoding="UTF-8"?>\n'
            printf '<bom xmlns="http://cyclonedx.org/schema/bom/1.5">\n'
            printf '  <metadata><tools><tool><name>gen-sbom.sh</name></tool></tools></metadata>\n'
            printf '  <externalReferences>\n'
            for f in "${OUTPUT_DIR}"/*/bom.xml; do
                [[ -f "$f" ]] || continue
                printf '    <reference type="bom"><url>%s</url></reference>\n' "${f#${OUTPUT_DIR}/}"
            done
            printf '  </externalReferences>\n</bom>\n'
        } > "${agg}"
    fi
    log "wrote aggregated ${agg}"
    log "done."
}

main "$@"
