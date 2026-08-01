#!/bin/bash
# selfsign.sh — de-quarantine and re-sign CopyPaste.app on the user's machine.
#
#   Usage: /bin/bash selfsign.sh <path-to-CopyPaste.app>
#
# This ships INSIDE the bundle, at Contents/Resources/selfsign.sh, and is run by
# the cask's postflight. It is not a release script: it runs on the machine that
# installed the app, as that machine's user, with no Apple account and no
# credential of any kind.
#
# Written for /bin/bash, which on macOS is 3.2. No associative arrays, no
# mapfile, no `readonly` arrays, and every empty-array expansion guarded — under
# `set -u` bash 3.2 treats "${arr[@]}" on an empty array as unbound.
#
# ---------------------------------------------------------------------------
# What it is for
# ---------------------------------------------------------------------------
# Two jobs, in this order, because the order matters:
#
#   1. Remove com.apple.quarantine, which Homebrew applies to everything it
#      downloads and which `--no-quarantine` no longer suppresses (deprecated in
#      Homebrew 5.1, no replacement). Removing extended attributes can break a
#      seal that covers them, so it happens BEFORE signing, not after.
#
#   2. Re-sign the bundle with a per-machine self-signed certificate, generated
#      once here and kept in a keychain in the user's home directory.
#
# Job 2 is the interesting one. macOS records a TCC grant (Accessibility and
# friends) against a code-signing requirement stored in the TCC database. For an
# ad-hoc signature that requirement is a cdhash, which changes on every build, so
# every update silently revokes the grant. For a signature made with a stable
# certificate the requirement is a hash of that certificate, which does not move
# — so a grant given once survives every later update signed by the same
# certificate. ADR-0001 has the evidence, read out of Apple's own source.
#
# CopyPaste needs no TCC permission today, so nothing currently depends on this.
# It exists so that auto-paste — the one feature that would need Accessibility —
# becomes possible without an Apple Developer ID.
#
# ---------------------------------------------------------------------------
# The contract with Homebrew
# ---------------------------------------------------------------------------
# `system_command` in a cask is `SystemCommand.run!`, which is must-succeed: a
# non-zero exit raises and Homebrew rolls the install back. So:
#
#   exit 0  — the bundle carries a valid signature. Either ours, or (if anything
#             about the certificate path failed) a plain ad-hoc one, which needs
#             no key and always works. The app launches either way.
#   exit 1  — the bundle does NOT carry a valid signature. This is the one case
#             where aborting the install beats completing it, because the
#             alternative is an app that cannot be opened.
#   exit 2  — called wrongly, or on the wrong platform.
#
# "A failed re-sign must not leave an unlaunchable app" is enforced by always
# having the ad-hoc fallback available, not by copying the bundle aside.
set -uo pipefail

CERT_CN="CopyPaste Local Signing"
BUNDLE_ID="com.copypaste.app"
CERT_DAYS=3650
INNER_BINARIES="copypaste copypaste-daemon"

APP="${1:-}"
if [ -z "$APP" ]; then
    echo "ERROR: usage: /bin/bash selfsign.sh <path-to-CopyPaste.app>" >&2
    exit 2
fi

# Signing material lives beside the app's own data so `zap` can remove it, and
# so there is exactly one directory to delete when starting over.
#
# `com.copypaste.CopyPaste` is CopyPaste's application-support directory on
# macOS.
DATA_DIR="${HOME}/Library/Application Support/com.copypaste.CopyPaste"
SUPPORT="${DATA_DIR}/signing"
KEYCHAIN_STEM="${SUPPORT}/copypaste-signing.keychain"
PASSFILE="${SUPPORT}/keychain-password"
FINGERPRINT_FILE="${SUPPORT}/certificate-sha1"

# CLAUDE.md rule 4: nothing shown to the user may contain a path that discloses
# the local username. Everything printed goes through this.
say() {
    local message="$*"
    printf '%s\n' "${message//${HOME}/\~}" >&2
}

# ---------------------------------------------------------------------------
# Preconditions
# ---------------------------------------------------------------------------
if [ "$(uname -s)" != "Darwin" ]; then
    say "ERROR: selfsign.sh runs on macOS only (this host is $(uname -s))."
    exit 2
fi
if [ ! -d "$APP" ]; then
    say "ERROR: no bundle at ${APP}"
    exit 2
fi
for tool in /usr/bin/codesign /usr/bin/security /usr/bin/openssl /usr/bin/xattr; do
    if [ ! -x "$tool" ]; then
        say "ERROR: ${tool} is missing; cannot continue."
        exit 2
    fi
done

# Escape hatch for the test procedure in ADR-0001, which installs the same build
# both ways and compares whether the Accessibility grant survived.
#   COPYPASTE_SIGN_MODE=adhoc       — skip the certificate entirely
#   COPYPASTE_SIGN_MODE=selfsigned  — fail loudly instead of falling back
SIGN_MODE="${COPYPASTE_SIGN_MODE:-auto}"
case "$SIGN_MODE" in
    auto|adhoc|selfsigned) ;;
    *) say "ERROR: COPYPASTE_SIGN_MODE must be auto, adhoc or selfsigned (got: ${SIGN_MODE})"; exit 2 ;;
esac

# ---------------------------------------------------------------------------
# 1. De-quarantine
# ---------------------------------------------------------------------------
# Only this bundle. The widely-copied `xattr -rd com.apple.quarantine
# /Applications/*` form de-quarantines every app the user has ever downloaded; a
# Homebrew maintainer objected to exactly that on the deprecation thread.
#
# The `|| true` is not laziness. `xattr -dr` exits non-zero when a file in the
# tree does not carry the attribute — the normal case for almost every file in a
# bundle — and under Homebrew's must-succeed `system_command` that non-zero exit
# would abort an otherwise fine install. The cask carried this call unguarded.
/usr/bin/xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

# ---------------------------------------------------------------------------
# 2. Signing
# ---------------------------------------------------------------------------
# Inner Mach-O binaries first, then the bundle. NOT `--deep`: Apple deprecated
# it, and it re-signs nested code with no identifier of its own, which is how
# identifiers end up rotating. `--identifier` pins one per binary so the
# designated requirement's identifier clause is stable too.
#
# --timestamp=none: a secure timestamp needs Apple's timestamp server. Requiring
# one would make an install fail whenever the network is unavailable, for no
# benefit on a signature Apple will never be asked about.
#
# Writes codesign's own output to stdout so the caller can inspect it; the
# caller decides what, if anything, the user sees.
sign_bundle() {
    local identity="$1"
    local keychain="${2:-}"
    local bin
    local -a keychain_args
    keychain_args=()
    if [ -n "$keychain" ]; then
        keychain_args=(--keychain "$keychain")
    fi

    for bin in $INNER_BINARIES; do
        [ -f "${APP}/Contents/MacOS/${bin}" ] || continue
        /usr/bin/codesign --force --sign "$identity" \
            --identifier "com.copypaste.${bin}" \
            --timestamp=none \
            ${keychain_args[@]+"${keychain_args[@]}"} \
            "${APP}/Contents/MacOS/${bin}" 2>&1 || return 1
    done

    /usr/bin/codesign --force --sign "$identity" \
        --identifier "$BUNDLE_ID" \
        --timestamp=none \
        ${keychain_args[@]+"${keychain_args[@]}"} \
        "$APP" 2>&1 || return 1

    /usr/bin/codesign --verify --strict "$APP" 2>&1 || return 1
    return 0
}

sign_adhoc() {
    sign_bundle - >/dev/null 2>&1
}

# ---------------------------------------------------------------------------
# 3. The keychain and the certificate
# ---------------------------------------------------------------------------
# A dedicated keychain rather than the login keychain, for one reason: this has
# to be non-interactive. A private key in the login keychain makes codesign
# raise a "wants to use a key in your keychain" dialog on first use, and the
# partition list that suppresses it can only be set with the login keychain's
# password — which we do not have and must not ask a postflight to collect.
#
# A keychain we create ourselves has a password we generate, so
# `set-key-partition-list` works and nothing ever prompts.
#
# The cost, stated rather than hidden: that password is a file in the user's home
# directory, so the private key is protected by file permissions rather than by
# the login password. This is a smaller step down than it looks. Code already
# running as this user can drive an unlocked login keychain too, and can rewrite
# the bundle in /Applications outright. What the key must not do is leave the
# machine, which is why it is imported non-extractable and never exported.

resolve_keychain_path() {
    # `security create-keychain` appends -db on current macOS but did not
    # always. Accept whichever exists rather than guessing.
    local candidate
    for candidate in "${KEYCHAIN_STEM}-db" "${KEYCHAIN_STEM}"; do
        if [ -f "$candidate" ]; then
            printf '%s' "$candidate"
            return 0
        fi
    done
    return 1
}

create_keychain() {
    local password kc
    password="$(/usr/bin/openssl rand -base64 32 | tr -d '\n')" || return 1
    /usr/bin/security create-keychain -p "$password" "$KEYCHAIN_STEM" >/dev/null 2>&1 || return 1
    kc="$(resolve_keychain_path)" || return 1

    # No auto-lock timeout and no lock on sleep. A locked keychain would make a
    # later upgrade fall back to ad-hoc and quietly lose the TCC grant, which is
    # the exact failure this whole path exists to prevent.
    /usr/bin/security set-keychain-settings "$kc" >/dev/null 2>&1 || true

    ( umask 077; printf '%s' "$password" > "$PASSFILE" ) || return 1
    return 0
}

# Prints the SHA-1 of our signing certificate, or nothing.
#
# Three forms, in order, and the order is the point. `-v -p codesigning` applies
# the code-signing policy, which may decline an identity whose root is not in
# any trust store; falling through to the unfiltered listing means an untrusted
# certificate is still found and reused rather than regenerated on every
# install, which would defeat the whole exercise.
find_identity_sha1() {
    local kc="$1"
    local listing sha1 args
    for args in "-v -p codesigning" "-v" ""; do
        # shellcheck disable=SC2086
        listing="$(/usr/bin/security find-identity $args "$kc" 2>/dev/null)"
        sha1="$(printf '%s\n' "$listing" | /usr/bin/awk -v cn="$CERT_CN" '
            index($0, cn) { for (i = 1; i <= NF; i++) if ($i ~ /^[0-9A-F]{40}$/) { print $i; exit } }')"
        if [ -n "$sha1" ]; then
            printf '%s' "$sha1"
            return 0
        fi
    done
    return 1
}

certificate_still_valid() {
    local kc="$1"
    /usr/bin/security find-certificate -c "$CERT_CN" -p "$kc" 2>/dev/null \
        | /usr/bin/openssl x509 -noout -checkend 86400 >/dev/null 2>&1
}

generate_certificate() {
    local kc="$1"
    local password="$2"
    local tmp status
    tmp="$(mktemp -d)" || return 1
    chmod 700 "$tmp"
    generate_certificate_inner "$kc" "$password" "$tmp"
    status=$?
    rm -rf "$tmp"
    return $status
}

generate_certificate_inner() {
    local kc="$1"
    local password="$2"
    local tmp="$3"
    local p12pass

    # A config file rather than `-addext`: macOS ships LibreSSL, whose support
    # for -addext varies by release, and a release-dependent flag in an install
    # path is a bug waiting for somebody else's machine.
    #
    # Organization is set deliberately. Apple's DRMaker walks up the chain from
    # the leaf looking for the first certificate with a different Organization;
    # with it set on a single self-signed certificate the generated requirement
    # pins the root, which is the form ADR-0001 quotes.
    cat > "${tmp}/openssl.cnf" <<EOF
[req]
distinguished_name = dn
prompt             = no
x509_extensions    = v3

[dn]
CN = ${CERT_CN}
O  = ${CERT_CN}

[v3]
basicConstraints     = critical,CA:false
keyUsage             = critical,digitalSignature
extendedKeyUsage     = critical,codeSigning
subjectKeyIdentifier = hash
EOF

    # /usr/bin/openssl explicitly, not `openssl`. A Homebrew OpenSSL 3 earlier on
    # PATH writes PKCS#12 with AES-256-CBC and PBKDF2 by default, which older
    # `security import` refuses — an install that fails only on machines that
    # happen to have another openssl installed.
    /usr/bin/openssl req -x509 -newkey rsa:2048 -sha256 -days "$CERT_DAYS" -nodes \
        -config "${tmp}/openssl.cnf" \
        -keyout "${tmp}/key.pem" -out "${tmp}/cert.pem" >/dev/null 2>&1 || return 1

    p12pass="$(/usr/bin/openssl rand -base64 24 | tr -d '\n')" || return 1
    /usr/bin/openssl pkcs12 -export \
        -inkey "${tmp}/key.pem" -in "${tmp}/cert.pem" \
        -name "$CERT_CN" -out "${tmp}/identity.p12" \
        -passout "pass:${p12pass}" >/dev/null 2>&1 || return 1

    # -x: the private key cannot be exported afterwards. It never needs to be —
    # it is per-machine by design and there is nothing to migrate it to.
    # -T /usr/bin/codesign: only codesign may use it.
    /usr/bin/security import "${tmp}/identity.p12" \
        -k "$kc" -f pkcs12 -P "$p12pass" -x -T /usr/bin/codesign \
        >/dev/null 2>&1 || return 1

    # The step that makes this non-interactive. Without it macOS prompts the
    # first time codesign touches the key, from inside a postflight, where there
    # is nobody to click the dialog.
    /usr/bin/security set-key-partition-list \
        -S apple-tool:,apple:,codesign: -s -k "$password" "$kc" \
        >/dev/null 2>&1 || return 1

    return 0
}

# Prints the identity SHA-1 on line 1 and the keychain path on line 2.
ensure_identity() {
    local kc password sha1
    mkdir -p "$SUPPORT" || return 1
    chmod 700 "$DATA_DIR" 2>/dev/null || true
    chmod 700 "$SUPPORT" 2>/dev/null || true

    kc="$(resolve_keychain_path)" || kc=""
    if [ -z "$kc" ] || [ ! -s "$PASSFILE" ]; then
        # Either there is no keychain, or there is one we can no longer unlock.
        # Both mean starting over; an unusable keychain left in place would make
        # every future upgrade fall back to ad-hoc for a reason nobody can see.
        if [ -n "$kc" ]; then
            /usr/bin/security delete-keychain "$kc" >/dev/null 2>&1 || rm -f "$kc"
        fi
        rm -f "$PASSFILE"
        create_keychain || return 1
        kc="$(resolve_keychain_path)" || return 1
    fi

    password="$(cat "$PASSFILE")" || return 1
    /usr/bin/security unlock-keychain -p "$password" "$kc" >/dev/null 2>&1 || return 1

    sha1="$(find_identity_sha1 "$kc")" || sha1=""
    if [ -n "$sha1" ] && ! certificate_still_valid "$kc"; then
        # Expired. Regenerating is correct, and it costs the user their grant, so
        # it is said out loud rather than done quietly.
        say "CopyPaste: the local signing certificate has expired; generating a new one."
        say "           macOS will ask again for any permission you had granted."
        sha1=""
    fi

    if [ -z "$sha1" ]; then
        generate_certificate "$kc" "$password" || return 1
        sha1="$(find_identity_sha1 "$kc")" || return 1
        [ -n "$sha1" ] || return 1
    fi

    printf '%s\n%s\n' "$sha1" "$kc"
    return 0
}

# ---------------------------------------------------------------------------
# 4. Run
# ---------------------------------------------------------------------------
fall_back_to_adhoc() {
    local reason="$1"

    if [ "$SIGN_MODE" = "selfsigned" ]; then
        say "ERROR: COPYPASTE_SIGN_MODE=selfsigned, and the certificate path failed:"
        say "       ${reason}"
        exit 1
    fi

    say "CopyPaste: could not sign with a local certificate — ${reason}."
    say "           Falling back to an ad-hoc signature. The app works either way, but"
    say "           any permission granted to it will have to be granted again after"
    say "           every update. 'brew reinstall --cask copypaste' retries this."

    if sign_adhoc; then
        exit 0
    fi

    say "ERROR: CopyPaste.app could not be signed at all and would not open."
    say "       The install was stopped rather than completed."
    exit 1
}

if [ "$SIGN_MODE" = "adhoc" ]; then
    if sign_adhoc; then
        exit 0
    fi
    say "ERROR: ad-hoc signing failed; CopyPaste.app would not open."
    exit 1
fi

IDENTITY_INFO="$(ensure_identity)" \
    || fall_back_to_adhoc "the signing certificate could not be created"
IDENTITY_SHA1="$(printf '%s\n' "$IDENTITY_INFO" | sed -n 1p)"
KEYCHAIN="$(printf '%s\n' "$IDENTITY_INFO" | sed -n 2p)"
if [ -z "$IDENTITY_SHA1" ] || [ -z "$KEYCHAIN" ]; then
    fall_back_to_adhoc "the signing certificate was not found after being created"
fi

# Sign by SHA-1 rather than by name: unambiguous if the user ever holds another
# certificate with a similar common name, and it does not go through the
# name-and-policy lookup that may decline an identity whose root is untrusted.
SIGN_OUTPUT="$(sign_bundle "$IDENTITY_SHA1" "$KEYCHAIN")" || {
    # If codesign declined the identity for trust reasons, say so precisely and
    # print the one command that would fix it — user trust domain, the user's own
    # password, no admin rights. Do not run it here: it prompts, and a postflight
    # has nobody to answer.
    if printf '%s\n' "$SIGN_OUTPUT" | grep -qi -e 'trust' -e 'unable to build chain'; then
        say "CopyPaste: macOS declined the local signing certificate as untrusted."
        say "           ADR-0001 records this as the open question this path was built"
        say "           to settle. To trust it for code signing (your own password, not"
        say "           an administrator's), see the ADR's test procedure."
    fi
    fall_back_to_adhoc "codesign rejected the certificate"
}

# Report a changed certificate. The only ways this happens are expiry and a
# reset of the signing directory, and both mean permissions have to be granted
# again — much less confusing when the installer says so.
PREVIOUS_SHA1=""
if [ -f "$FINGERPRINT_FILE" ]; then
    PREVIOUS_SHA1="$(cat "$FINGERPRINT_FILE" 2>/dev/null)" || PREVIOUS_SHA1=""
fi
if [ -n "$PREVIOUS_SHA1" ] && [ "$PREVIOUS_SHA1" != "$IDENTITY_SHA1" ]; then
    say "CopyPaste: the local signing certificate changed, so macOS treats this as a"
    say "           different app. Any permission granted before must be granted again."
fi
( umask 077; printf '%s' "$IDENTITY_SHA1" > "$FINGERPRINT_FILE" ) || true

exit 0
