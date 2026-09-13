#!/usr/bin/env bash
# Create a GitHub Release without assets, then upload them.
#
# POST /releases has returned HTTP 500 after persisting the release
# (run 34746947087). A later create then fails with already-exists and
# never uploads, so the tap job is skipped. Treat a live tag as success
# and only fail when create errors and the release is still missing.
set -euo pipefail

die() { echo "ERROR: $*" >&2; exit 1; }

VERSION=""
REPO=""
NOTES=""
ASSETS=()

release_exists() {
  gh release view "$1" --repo "$2" >/dev/null 2>&1
}

publish() {
  local tag="v${VERSION}"
  local asset create

  [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]] \
    || die "version is not a valid release version"
  [[ "$REPO" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] \
    || die "repo must be OWNER/NAME"
  [[ -s "$NOTES" ]] || die "release notes file is missing or empty"
  ((${#ASSETS[@]} > 0)) || die "at least one release asset is required"
  for asset in "${ASSETS[@]}"; do
    [[ -s "$asset" ]] || die "release asset is missing or empty"
  done

  if ! release_exists "$tag" "$REPO"; then
    create=(
      gh release create "$tag"
      --repo "$REPO"
      --title "$tag"
      --notes-file "$NOTES"
      --verify-tag
    )
    if [[ "$VERSION" == *-* ]]; then
      create+=(--prerelease)
    else
      create+=(--latest)
    fi
    if ! "${create[@]}"; then
      release_exists "$tag" "$REPO" \
        || die "gh release create failed and the release does not exist"
    fi
  fi

  gh release upload "$tag" --repo "$REPO" --clobber "${ASSETS[@]}"
}

self_test() {
  local root stub_bin log notes asset script
  root=$(mktemp -d)
  trap 'rm -rf "$root"' RETURN
  stub_bin="$root/bin"
  mkdir -p "$stub_bin"
  log="$root/gh.log"
  notes="$root/notes.md"
  asset="$root/app.exe"
  script=$0
  printf 'notes\n' >"$notes"
  printf 'exe\n' >"$asset"

  cat >"$stub_bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"${GH_STUB_LOG:?}"
[[ ${1:-} == release ]] || { echo "unexpected: $*" >&2; exit 1; }
shift
case "${1:-}" in
  create)
    case "${GH_STUB_CREATE:-ok}" in
      fail500)
        echo "HTTP 500 (https://api.github.com/repos/owner/repo/releases)" >&2
        exit 1
        ;;
      fail422)
        echo "HTTP 422: already exists" >&2
        exit 1
        ;;
      ok) exit 0 ;;
      *) echo "unknown create mode" >&2; exit 1 ;;
    esac
    ;;
  view)
    case "${GH_STUB_VIEW:-missing}" in
      exists) exit 0 ;;
      exists-after-create)
        if [[ "$(grep -c '^release view ' "${GH_STUB_LOG}")" -le 1 ]]; then
          echo "release not found" >&2
          exit 1
        fi
        exit 0
        ;;
      missing)
        echo "release not found" >&2
        exit 1
        ;;
      *) echo "unknown view mode" >&2; exit 1 ;;
    esac
    ;;
  upload)
    [[ "${GH_STUB_UPLOAD:-ok}" == ok ]] || { echo "upload failed" >&2; exit 1; }
    exit 0
    ;;
  *)
    echo "unexpected release subcommand: $*" >&2
    exit 1
    ;;
esac
EOF
  chmod +x "$stub_bin/gh"

  run_pub() {
    : >"$log"
    env PATH="$stub_bin:$PATH" GH_STUB_LOG="$log" \
      GH_STUB_CREATE="${GH_STUB_CREATE:-ok}" \
      GH_STUB_VIEW="${GH_STUB_VIEW:-missing}" \
      GH_STUB_UPLOAD="${GH_STUB_UPLOAD:-ok}" \
      "$script" --version "${PUB_VERSION:-2.0.0-alpha.35}" --repo owner/repo \
      --notes "$notes" "$asset"
  }

  GH_STUB_CREATE=ok GH_STUB_VIEW=missing run_pub \
    || die "self-test failed: create then upload"
  grep -q 'release create v2.0.0-alpha.35' "$log" \
    || die "self-test failed: create not invoked"
  grep -q -- '--verify-tag' "$log" \
    || die "self-test failed: create must verify the tag"
  grep -q -- '--prerelease' "$log" \
    || die "self-test failed: prerelease flag"
  grep -q 'release upload v2.0.0-alpha.35' "$log" \
    || die "self-test failed: upload not invoked"
  grep -q -- '--clobber' "$log" \
    || die "self-test failed: upload must clobber"

  GH_STUB_VIEW=exists run_pub \
    || die "self-test failed: existing release"
  grep -q 'release create' "$log" \
    && die "self-test failed: existing release must not create"
  grep -q 'release upload' "$log" \
    || die "self-test failed: existing release must upload"

  GH_STUB_CREATE=fail500 GH_STUB_VIEW=exists-after-create run_pub \
    || die "self-test failed: persisted release after create 500"
  grep -q 'release upload' "$log" \
    || die "self-test failed: persisted release must upload"

  if GH_STUB_CREATE=fail500 GH_STUB_VIEW=missing run_pub; then
    die "self-test accepted a create 500 with no release"
  fi
  grep -q 'release upload' "$log" \
    && die "self-test uploaded after a missing release"

  GH_STUB_CREATE=fail422 GH_STUB_VIEW=exists-after-create run_pub \
    || die "self-test failed: create already-exists then view"

  if env PATH="$stub_bin:$PATH" GH_STUB_LOG="$log" \
      "$script" --version 2.0.0-alpha.35 --repo owner/repo \
      --notes "$root/missing.md" "$asset"; then
    die "self-test accepted missing notes"
  fi
  if env PATH="$stub_bin:$PATH" GH_STUB_LOG="$log" \
      "$script" --version 2.0.0-alpha.35 --repo owner/repo \
      --notes "$notes" "$root/missing.exe"; then
    die "self-test accepted a missing asset"
  fi
  if env PATH="$stub_bin:$PATH" GH_STUB_LOG="$log" \
      "$script" --version 2.0.0-alpha.35 --repo owner/repo --notes "$notes"; then
    die "self-test accepted no assets"
  fi

  PUB_VERSION=2.0.0 GH_STUB_CREATE=ok GH_STUB_VIEW=missing run_pub \
    || die "self-test failed: stable create"
  grep -q -- '--latest' "$log" \
    || die "self-test failed: stable release must be latest"
  grep -q -- '--prerelease' "$log" \
    && die "self-test failed: stable release must not be prerelease"

  echo "PASS: publish-github-release self-test"
}

if [[ "${1:-}" == --self-test ]]; then
  [[ $# -eq 1 ]] || die "usage: $0 --self-test"
  self_test
  exit 0
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || die "--version needs a value"
      VERSION=$2
      shift 2
      ;;
    --repo)
      [[ $# -ge 2 ]] || die "--repo needs a value"
      REPO=$2
      shift 2
      ;;
    --notes)
      [[ $# -ge 2 ]] || die "--notes needs a value"
      NOTES=$2
      shift 2
      ;;
    --)
      shift
      ASSETS+=("$@")
      break
      ;;
    -*)
      die "unknown argument: $1"
      ;;
    *)
      ASSETS+=("$1")
      shift
      ;;
  esac
done

[[ -n "$VERSION" ]] || die "--version is required"
[[ -n "$REPO" ]] || die "--repo is required"
[[ -n "$NOTES" ]] || die "--notes is required"
publish
