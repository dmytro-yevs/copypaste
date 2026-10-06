#!/usr/bin/env bash
set -euo pipefail

APK="${1:-}"
VERSION="${2:-}"
EXPECTED_VERSION_CODE="${3:-}"
EXPECTED_CERT="${4:-}"
AAPT2="${AAPT2:-}"
APKSIGNER="${APKSIGNER:-}"
APK_ANALYZER="${APK_ANALYZER:-$(dirname "$(dirname "$(dirname "$AAPT2")")")/cmdline-tools/latest/bin/apkanalyzer}"

[[ -f "$APK" && "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ && "$EXPECTED_VERSION_CODE" =~ ^[1-9][0-9]*$ ]] || {
    echo "ERROR: usage: $0 <apk> <stable-version> <version-code> <certificate-sha256>" >&2
    exit 1
}
[[ "$EXPECTED_CERT" =~ ^[a-fA-F0-9]{64}$ ]] || {
    echo "ERROR: expected certificate fingerprint must be 64 hexadecimal characters" >&2
    exit 1
}
[[ -x "$AAPT2" && -x "$APKSIGNER" && -x "$APK_ANALYZER" ]] || {
    echo "ERROR: AAPT2, APKSIGNER, and APK_ANALYZER must name executable Android SDK tools" >&2
    exit 1
}

signature="$($APKSIGNER verify --verbose --print-certs-pem "$APK")"
grep -q '^Verifies$' <<<"$signature" || {
    echo "ERROR: APK signature verification failed" >&2
    exit 1
}
pem="$(awk '/-----BEGIN CERTIFICATE-----/{found=1} found{print} /-----END CERTIFICATE-----/{exit}' <<<"$signature")"
[[ -n "$pem" ]] || {
    echo "ERROR: APK signer certificate is missing" >&2
    exit 1
}
actual_cert="$(openssl x509 -outform DER <<<"$pem" | sha256sum | cut -d' ' -f1)"
[[ "$actual_cert" == "$(tr '[:upper:]' '[:lower:]' <<<"$EXPECTED_CERT")" ]] || {
    echo "ERROR: APK signer does not match the pinned production certificate: expected=$EXPECTED_CERT actual=$actual_cert" >&2
    exit 1
}

badging="$($AAPT2 dump badging "$APK")"
grep -q "package: name='com.copypaste.app'" <<<"$badging" || {
    echo "ERROR: APK application id is not com.copypaste.app" >&2
    exit 1
}
grep -q "versionName='$VERSION'" <<<"$badging" || {
    echo "ERROR: APK versionName does not match $VERSION" >&2
    exit 1
}
grep -q "versionCode='$EXPECTED_VERSION_CODE'" <<<"$badging" || {
    echo "ERROR: APK versionCode does not match $EXPECTED_VERSION_CODE" >&2
    exit 1
}
if grep -q '^application-debuggable' <<<"$badging"; then
    echo "ERROR: production APK is debuggable" >&2
    exit 1
fi

for abi in armeabi-v7a arm64-v8a x86_64; do
    unzip -Z1 "$APK" | grep "^lib/$abi/.*\.so$" > /dev/null || {
        echo "ERROR: production APK is missing $abi native libraries" >&2
        exit 1
    }
done

for registrar in \
    com.google.mlkit.common.internal.CommonComponentRegistrar \
    com.google.mlkit.vision.common.internal.VisionCommonRegistrar; do
    bytecode="$("$APK_ANALYZER" dex code --class "$registrar" "$APK")"
    grep -Eq '^\.method public .*<init>\(\)V$' <<<"$bytecode" || {
        echo "ERROR: APK is missing the public constructor for $registrar" >&2
        exit 1
    }
done

directory="$(cd "$(dirname "$APK")" && pwd)"
filename="$(basename "$APK")"
(cd "$directory" && sha256sum "$filename" > "$filename.sha256")
printf 'verified Android release %s (%s)\n' "$filename" "$actual_cert"
