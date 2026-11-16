#!/bin/sh
# Install CopyPaste for macOS from GitHub Releases.
#
#   curl -fsSL https://raw.githubusercontent.com/dmytro-yevs/copypaste/main/scripts/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --version v0.1.0 --dir "$HOME/Applications" --dry-run
#
# The DMG must match the release's published .sha256 file. The app goes into
# /Applications, or ~/Applications when /Applications is not writable, and is
# re-signed with the same per-machine identity the Homebrew cask uses. curl does
# not quarantine what it downloads, so Gatekeeper does not block the first launch
# of this unnotarized build. The script never uses sudo and never changes
# system-wide Gatekeeper settings.
set -eu

REPO="dmytro-yevs/copypaste"
APP="CopyPaste.app"
MIN_MACOS_MAJOR=14

work=""
mount=""
staged=""

say() { printf 'copypaste-install: %s\n' "$*"; }
fail() {
    printf 'copypaste-install: error: %s\n' "$*" >&2
    exit 1
}

usage() {
    cat <<EOF
Usage: install.sh [--version vX.Y.Z] [--dir DIRECTORY] [--dry-run]

Installs $APP from https://github.com/$REPO/releases.

  --version TAG   Release tag to install (default: latest release).
  --dir DIR       Install directory (default: /Applications, or
                  ~/Applications when /Applications is not writable).
  --dry-run       Download and verify the release, then exit without
                  installing anything.

COPYPASTE_VERSION and COPYPASTE_INSTALL_DIR set the same defaults.
EOF
}

cleanup() {
    if [ -n "$mount" ]; then
        hdiutil detach "$mount" -quiet >/dev/null 2>&1 || true
    fi
    if [ -n "$staged" ]; then
        rm -rf "$staged"
    fi
    if [ -n "$work" ]; then
        rm -rf "$work"
    fi
}

require() {
    for tool in "$@"; do
        command -v "$tool" >/dev/null 2>&1 || fail "$tool is required but was not found"
    done
}

check_platform() {
    [ "$(uname -s)" = Darwin ] ||
        fail "this installer supports macOS only; get other platforms from https://github.com/$REPO/releases"
    require curl shasum hdiutil ditto codesign sw_vers sysctl pgrep
    major="$(sw_vers -productVersion | cut -d. -f1)"
    [ "$major" -ge "$MIN_MACOS_MAJOR" ] 2>/dev/null ||
        fail "CopyPaste needs macOS $MIN_MACOS_MAJOR or later (this Mac runs $(sw_vers -productVersion))"
    # uname -m reports x86_64 under Rosetta, so ask the hardware instead.
    [ "$(sysctl -n hw.optional.arm64 2>/dev/null || echo 0)" = 1 ] ||
        fail "releases are built for Apple silicon only; Intel Macs are not supported"
    arch=arm64
}

resolve_version() {
    if [ -z "$version" ]; then
        latest="$(curl -fsSLI --proto '=https' -o /dev/null -w '%{url_effective}' \
            "https://github.com/$REPO/releases/latest")" ||
            fail "could not reach GitHub to find the latest release"
        version="${latest##*/tag/}"
        [ "$version" != "$latest" ] || fail "could not resolve the latest release from $latest"
    fi
    version="v${version#v}"
    printf '%s\n' "$version" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$' ||
        fail "invalid release tag: $version"
}

download_and_verify() {
    asset="CopyPaste-$version-macos-$arch.dmg"
    base="https://github.com/$REPO/releases/download/$version"
    say "downloading $asset from $base"
    curl -fL --proto '=https' --progress-bar -o "$work/$asset" "$base/$asset" ||
        fail "could not download $base/$asset"
    curl -fsSL --proto '=https' -o "$work/$asset.sha256" "$base/$asset.sha256" ||
        fail "release $version publishes no $asset.sha256; refusing to install an unverified download"

    expected="$(awk 'NR == 1 { print tolower($1) }' "$work/$asset.sha256")"
    printf '%s\n' "$expected" | grep -Eq '^[0-9a-f]{64}$' || fail "$asset.sha256 is not a SHA-256 checksum"
    actual="$(shasum -a 256 "$work/$asset" | awk '{ print $1 }')"
    [ "$expected" = "$actual" ] || fail "checksum mismatch for $asset (expected $expected, got $actual)"
    say "SHA-256 verified: $actual"

    mount="$work/volume"
    mkdir "$mount"
    hdiutil attach "$work/$asset" -nobrowse -readonly -noautoopen -mountpoint "$mount" -quiet ||
        fail "could not mount $asset"
    [ -d "$mount/$APP" ] || fail "$asset does not contain $APP"
    codesign --verify --strict "$mount/$APP" || fail "the code signature of $APP in $asset is invalid"
    [ -f "$mount/$APP/Contents/Resources/selfsign.sh" ] || fail "$APP in $asset has no local signing script"
}

choose_destination() {
    if [ -z "$dest" ]; then
        if [ -w /Applications ]; then
            dest=/Applications
        else
            dest="$HOME/Applications"
        fi
    fi
    case "$dest" in
        /*) ;;
        *) fail "--dir must be an absolute path (got: $dest)" ;;
    esac
}

install_app() {
    mkdir -p "$dest" || fail "could not create $dest"
    [ -w "$dest" ] || fail "$dest is not writable; pass --dir \"\$HOME/Applications\" instead of using sudo"

    staged="$dest/.CopyPaste.install.$$.app"
    say "copying $APP into $dest"
    ditto "$mount/$APP" "$staged" || fail "could not copy $APP into $dest"

    # The bundled script is what the Homebrew cask runs: it re-signs with a
    # stable local identity so Keychain and Accessibility grants survive updates.
    say "signing $APP with the local CopyPaste identity"
    /bin/bash "$staged/Contents/Resources/selfsign.sh" "$staged" ||
        fail "local signing failed; the existing installation was left unchanged"

    if [ -e "$dest/$APP" ]; then
        previous="$dest/.CopyPaste.previous.$$.app"
        mv "$dest/$APP" "$previous" || fail "could not move the existing $dest/$APP aside"
        if ! mv "$staged" "$dest/$APP"; then
            mv "$previous" "$dest/$APP"
            fail "could not replace $dest/$APP; the previous version was restored"
        fi
        rm -rf "$previous"
        say "replaced the previous $dest/$APP"
    else
        mv "$staged" "$dest/$APP" || fail "could not move $APP into $dest"
    fi
    staged=""
}

main() {
    version="${COPYPASTE_VERSION:-}"
    dest="${COPYPASTE_INSTALL_DIR:-}"
    dry_run=0
    while [ $# -gt 0 ]; do
        case "$1" in
            --version) [ $# -ge 2 ] || fail "--version needs a value"; version="$2"; shift 2 ;;
            --version=*) version="${1#*=}"; shift ;;
            --dir) [ $# -ge 2 ] || fail "--dir needs a value"; dest="$2"; shift 2 ;;
            --dir=*) dest="${1#*=}"; shift ;;
            --dry-run) dry_run=1; shift ;;
            -h | --help) usage; exit 0 ;;
            *) fail "unknown option: $1 (see --help)" ;;
        esac
    done

    check_platform
    resolve_version
    choose_destination
    work="$(mktemp -d "${TMPDIR:-/tmp}/copypaste-install.XXXXXX")" || fail "could not create a temporary directory"
    trap cleanup EXIT
    trap 'exit 130' INT TERM HUP

    say "installing CopyPaste $version ($arch) into $dest"
    download_and_verify

    running=0
    pgrep -f "$dest/$APP/Contents/MacOS/" >/dev/null 2>&1 && running=1

    if [ "$dry_run" = 1 ]; then
        [ "$running" = 0 ] || say "dry run: CopyPaste is running from $dest; quit it before installing"
        say "dry run: $asset is verified and contains a validly signed $APP"
        say "dry run: would install $dest/$APP and sign it locally; nothing was changed"
        return 0
    fi

    [ "$running" = 0 ] || fail "CopyPaste is running from $dest/$APP; quit it and run the installer again"
    install_app
    say "installed CopyPaste $version at $dest/$APP"
    say "open it with: open \"$dest/$APP\""
}

main "$@"
