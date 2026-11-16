#!/usr/bin/env bash
set -euo pipefail
# Submit the exact qualified Windows installer to the winget community repository.
version="${1:?usage: publish-winget.sh version installer}"
installer="${2:?usage: publish-winget.sh version installer}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
identifier="CopyPaste.CopyPaste"
manifests="https://api.github.com/repos/microsoft/winget-pkgs/contents/manifests/c/CopyPaste/CopyPaste"

if [[ -z "${WINGET_TOKEN:-}" ]]; then
  echo "::notice::WINGET_TOKEN is not configured; skipping winget submission"
  exit 0
fi
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
[[ -f "$installer" ]]

manifest_status() {
  curl --silent --show-error --output /dev/null --write-out '%{http_code}' \
    --header "Authorization: Bearer $WINGET_TOKEN" --header "Accept: application/vnd.github+json" "$1"
}
case "$(manifest_status "$manifests")" in
  200) ;;
  404)
    echo "ERROR: $identifier is not in winget-pkgs yet; submit its first manifest manually" >&2
    exit 1
    ;;
  *) echo "ERROR: could not query winget-pkgs for $identifier" >&2; exit 1 ;;
esac
if [[ "$(manifest_status "$manifests/$version")" == 200 ]]; then
  echo "$identifier $version is already in winget-pkgs"
  exit 0
fi

temporary="$(mktemp -d)"
trap 'rm -rf "$temporary"' EXIT
# The published release asset must be byte-identical to the qualified installer.
url="https://github.com/$GITHUB_REPOSITORY/releases/download/v$version/$(basename "$installer")"
curl --fail --location --silent --show-error "$url" --output "$temporary/published.exe"
expected="$(sha256sum "$installer" | cut -d' ' -f1)"
printf '%s  %s\n' "$expected" "$temporary/published.exe" | sha256sum --check --status || {
  echo "ERROR: published Windows installer does not match the qualified artifact" >&2
  exit 1
}

# Komac is pinned by version and checksum, not installed from a moving channel.
curl --fail --location --silent --show-error \
  https://github.com/russellbanks/Komac/releases/download/v2.17.1/komac-2.17.1-x86_64-unknown-linux-gnu.tar.gz \
  --output "$temporary/komac.tar.gz"
printf '%s  %s\n' e694e79cd451afabf47bead66ed34586fef4814f7b94e4b61a098f419978d5d7 "$temporary/komac.tar.gz" | sha256sum --check --status
tar -xzf "$temporary/komac.tar.gz" -C "$temporary" komac

GITHUB_TOKEN="$WINGET_TOKEN" "$temporary/komac" update "$identifier" \
  --version "$version" --urls "$url" \
  --release-notes-url "https://github.com/$GITHUB_REPOSITORY/releases/tag/v$version" \
  --submit
