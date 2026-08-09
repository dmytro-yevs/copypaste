#!/usr/bin/env bash
# check.sh — everything about the release pipeline that can be checked off a Mac.
#
#   Usage: scripts/release/check.sh
#
# The macOS half of this pipeline has never been executed. That makes the
# question "what do we actually know?" worth answering mechanically rather than
# by reading, so this runs on Linux, in CI or by hand, and asserts the parts
# that do not need Darwin:
#
#   * every shell script parses, under the shell that will run it
#   * the cask and the formula are valid Ruby
#   * the generators rewrite exactly the two lines they claim to, are idempotent,
#     leave valid Ruby behind, and reject the inputs they say they reject
#   * the generated tap layout is the one `brew tap` expects
#   * the workflows are wired to each other correctly — check-wiring.py
#
# What it deliberately does NOT do is pretend to check codesign, hdiutil,
# PlistBuddy, the Tauri bundler or whether the bundle launches. Those need a Mac
# and are marked UNVERIFIED in the scripts themselves.
#
# Exit 0 = every check passed. Exit 1 = at least one failed, and each failure is
# printed with what was expected.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

PASS=0
FAIL=0

ok()   { PASS=$((PASS + 1)); printf '  ok    %s\n' "$1"; }
bad()  { FAIL=$((FAIL + 1)); printf '  FAIL  %s\n' "$1"; [[ -n "${2:-}" ]] && printf '        %s\n' "$2"; }
group() { printf '\n== %s\n' "$1"; }

# check <description> <command...>
check() {
    local desc="$1"; shift
    local out
    if out="$("$@" 2>&1)"; then
        ok "$desc"
    else
        bad "$desc" "${out:-(no output)}"
    fi
}

group "Shell syntax"
for f in scripts/release/*.sh; do
    check "bash -n $f" bash -n "$f"
done
# selfsign.sh runs under macOS /bin/bash, which is 3.2. `bash -n` here cannot
# prove 3.2 compatibility, only that it parses; the script's header records the
# constructs that were avoided and why.
check "bash -n packaging/macos/selfsign.sh" bash -n packaging/macos/selfsign.sh

for f in scripts/release/*.sh packaging/macos/selfsign.sh; do
    mode="$(git ls-files --stage -- "$f" | awk '{print $1}')"
    if [[ "$mode" == 100755 ]]; then
        ok "executable bit set on $f"
    else
        bad "executable bit set on $f" "git update-index --chmod=+x -- $f"
    fi
done

if command -v shellcheck >/dev/null 2>&1; then
    for f in scripts/release/*.sh packaging/macos/selfsign.sh; do
        check "shellcheck $f" shellcheck -S warning "$f"
    done
else
    printf '  skip  shellcheck (not installed)\n'
fi

group "Ruby syntax"
if command -v ruby >/dev/null 2>&1; then
    check "ruby -c Casks/copypaste.rb"                    ruby -c Casks/copypaste.rb
    check "ruby -c packaging/homebrew/copypaste-cli.rb"   ruby -c packaging/homebrew/copypaste-cli.rb
else
    printf '  skip  ruby (not installed)\n'
fi

group "Install-time tooling uses the application data directory"
for f in Casks/copypaste.rb packaging/macos/selfsign.sh; do
    if grep -qE 'Application Support/CopyPaste([/"]|$)' "$f"; then
        bad "$f names only the application data directory" \
            "it names an unsupported application-support directory"
    else
        ok "$f names only the application data directory"
    fi
done
if grep -q 'Application Support/com.copypaste.CopyPaste' Casks/copypaste.rb; then
    ok "the cask zaps the application data directory"
else
    bad "the cask zaps the application data directory" \
        "zap removes no CopyPaste data at all"
fi

group "Seeded values fail closed"
# An unreleased checkout must not be able to install anything: version 0.0.0 has
# no release, and an all-zero checksum cannot match a real file.
ZERO="0000000000000000000000000000000000000000000000000000000000000000"
for f in Casks/copypaste.rb packaging/homebrew/copypaste-cli.rb; do
    if grep -qE '^[[:space:]]*version "0\.0\.0"$' "$f" && grep -q "$ZERO" "$f"; then
        ok "$f is seeded with 0.0.0 and an all-zero sha256"
    else
        bad "$f is seeded with 0.0.0 and an all-zero sha256" \
            "a real version is checked in — the generators rewrite in place, so this file should be committed reset"
    fi
done

group "Generators round-trip"
SHA_A="1111111111111111111111111111111111111111111111111111111111111111"
SHA_B="2222222222222222222222222222222222222222222222222222222222222222"
VERSION_A="2.0.0-alpha.1"
TAPDIR="$(mktemp -d)"
BACKUP="$(mktemp -d)"
cp Casks/copypaste.rb "$BACKUP/cask.rb"
cp packaging/homebrew/copypaste-cli.rb "$BACKUP/formula.rb"
restore() {
    cp "$BACKUP/cask.rb"    Casks/copypaste.rb
    cp "$BACKUP/formula.rb" packaging/homebrew/copypaste-cli.rb
    rm -rf "$TAPDIR" "$BACKUP"
}
trap restore EXIT

check "gen-cask.sh accepts a pre-release version"    ./scripts/release/gen-cask.sh    "$VERSION_A" "$SHA_A" --out "$TAPDIR"
check "gen-formula.sh accepts a pre-release version" ./scripts/release/gen-formula.sh "$VERSION_A" "$SHA_B" --out "$TAPDIR"

for pair in "Casks/copypaste.rb:$SHA_A" "packaging/homebrew/copypaste-cli.rb:$SHA_B"; do
    f="${pair%%:*}"; sha="${pair##*:}"
    if grep -qE "^[[:space:]]*version \"${VERSION_A}\"$" "$f"; then
        ok "$f version rewritten"
    else
        bad "$f version rewritten"
    fi
    if [[ "$(grep -cE "^[[:space:]]*sha256 \"${sha}\"$" "$f")" == "1" ]]; then
        ok "$f sha256 rewritten exactly once"
    else
        bad "$f sha256 rewritten exactly once" \
            "found $(grep -cE "^[[:space:]]*sha256 " "$f") sha256 lines"
    fi
    if grep -qE "^[[:space:]]*version \"0\.0\.0\"$" "$f"; then
        bad "$f has no leftover seed version"
    else
        ok "$f has no leftover seed version"
    fi
done

if command -v ruby >/dev/null 2>&1; then
    check "stamped cask is still valid Ruby"    ruby -c Casks/copypaste.rb
    check "stamped formula is still valid Ruby" ruby -c packaging/homebrew/copypaste-cli.rb
fi

# Idempotence. The release workflow can be re-run on the same tag, and a
# generator that only works on a pristine file would corrupt the second run.
check "gen-cask.sh is idempotent"    ./scripts/release/gen-cask.sh    "$VERSION_A" "$SHA_A" --out "$TAPDIR"
check "gen-formula.sh is idempotent" ./scripts/release/gen-formula.sh "$VERSION_A" "$SHA_B" --out "$TAPDIR"
if [[ "$(grep -cE "^[[:space:]]*version \"${VERSION_A}\"$" Casks/copypaste.rb)" == "1" ]]; then
    ok "cask still has exactly one version line after a second run"
else
    bad "cask still has exactly one version line after a second run"
fi

# The URL the cask builds must be the filename make-dmg.sh writes. This is the
# join between two scripts that never see each other.
EXPECTED_DMG="CopyPaste-v${VERSION_A}-macos-arm64.dmg"
if grep -q 'CopyPaste-v#{version}-macos-arm64\.dmg' Casks/copypaste.rb; then
    ok "cask URL interpolates to $EXPECTED_DMG"
else
    bad "cask URL interpolates to $EXPECTED_DMG" "the url stanza does not match the DMG naming in make-dmg.sh"
fi
EXPECTED_TGZ="copypaste-cli-v${VERSION_A}-macos-arm64.tar.gz"
if grep -q 'copypaste-cli-v#{version}-macos-arm64\.tar\.gz' packaging/homebrew/copypaste-cli.rb; then
    ok "formula URL interpolates to $EXPECTED_TGZ"
else
    bad "formula URL interpolates to $EXPECTED_TGZ"
fi

group "Generators reject bad input"
reject() {
    local desc="$1"; shift
    if "$@" >/dev/null 2>&1; then
        bad "$desc" "the command succeeded and should not have"
    else
        ok "$desc"
    fi
}
reject "gen-cask.sh rejects a leading v"        ./scripts/release/gen-cask.sh    "v2.0.0" "$SHA_A"
reject "gen-formula.sh rejects a leading v"     ./scripts/release/gen-formula.sh "v2.0.0" "$SHA_A"
reject "gen-cask.sh rejects a short sha256"     ./scripts/release/gen-cask.sh    "2.0.0"  "deadbeef"
reject "gen-cask.sh rejects uppercase sha256"   ./scripts/release/gen-cask.sh    "2.0.0"  "$(printf '%s' "$SHA_A" | tr '12' 'AB')"
reject "gen-cask.sh rejects a missing argument" ./scripts/release/gen-cask.sh    "2.0.0"
reject "gen-cask.sh rejects an unknown flag"    ./scripts/release/gen-cask.sh    "2.0.0" "$SHA_A" --nope

group "Tap layout"
# `brew tap` looks for Casks/ and Formula/ at the repository root.
for p in "Casks/copypaste.rb" "Formula/copypaste-cli.rb"; do
    if [[ -f "$TAPDIR/$p" ]]; then
        ok "generated tap contains $p"
    else
        bad "generated tap contains $p" "found: $(cd "$TAPDIR" && find . -type f | sort | tr '\n' ' ')"
    fi
done
# A maintainer may already have the sibling tap checkout. Dry-run must remain
# read-only and successful in that ordinary state, otherwise this checker is
# not repeatable on the machine that publishes releases.
check "setup-tap.sh --dry-run is idempotent beside an existing tap" \
    ./scripts/release/setup-tap.sh --github-user dmytro-yevs --dry-run
reject "setup-tap.sh needs a user"   ./scripts/release/setup-tap.sh --dry-run
reject "setup-tap.sh rejects a bad tap name" ./scripts/release/setup-tap.sh --github-user dmytro-yevs --tap-name "Bad Name" --dry-run

group "One version, three files"
# Cargo metadata is authoritative. Tauri still resolves package.json for its
# other bundlers, so the Android resolver fails unless all three paths agree.
WS_V="$(awk '/^\[workspace\.package\]/{f=1} f && /^version *=/{gsub(/[" ]/,"",$3); print $3; exit}' Cargo.toml)"
PKG_V="$(python3 -c 'import json;print(json.load(open("crates/copypaste-ui/package.json"))["version"])' 2>/dev/null || true)"
CONF_V="$(python3 -c 'import json;print(json.load(open("crates/copypaste-ui/src-tauri/tauri.conf.json")).get("version",""))' 2>/dev/null || true)"
if [[ -n "$WS_V" && "$WS_V" == "$PKG_V" ]]; then
    ok "Cargo.toml [workspace.package] and package.json agree on $WS_V"
else
    bad "Cargo.toml [workspace.package] and package.json agree" \
        "workspace=$WS_V package.json=$PKG_V — Android metadata rejects a mismatch"
fi

check "Android release metadata preserves prerelease ordering" \
    node --test scripts/release/android-metadata.test.mjs
check "Cargo product metadata derivatives are current" \
    node scripts/release/android-metadata.mjs --check
check "Android artifact identity checks fail closed" \
    bash scripts/release/android-artifact-check.sh --self-test
if grep -Fq '"debugApplicationIdSuffix":' crates/copypaste-ui/src-tauri/tauri.android.conf.json \
        && grep -Fq '"versionCode":' crates/copypaste-ui/src-tauri/tauri.android.conf.json \
        && grep -Fq 'val tauriProperties = Properties().apply' crates/copypaste-ui/src-tauri/gen/android/app/build.gradle.kts \
        && grep -Fq 'applicationIdSuffix = ".debug"' crates/copypaste-ui/src-tauri/gen/android/app/build.gradle.kts \
        && ! grep -q 'androidMetadata' crates/copypaste-ui/src-tauri/gen/android/app/build.gradle.kts; then
    ok "Tauri Android config resolves identity and version from release metadata"
else
    bad "Tauri Android config resolves identity and version from release metadata"
fi
if grep -q 'android-artifact-check.sh' .github/workflows/android-emulator.yml \
        && grep -q 'android-install-upgrade.sh' scripts/release/android-release-emulator-legs.sh \
        && grep -q -- '--write-overlay "$previous_config"' .github/workflows/android-emulator.yml \
        && grep -q -- '--config "$previous_config"' .github/workflows/android-emulator.yml \
        && grep -q 'name: android-upgrade-fixture' .github/workflows/release.yml \
        && grep -q 'PREVIOUS_APK: upgrade-dist/copypaste-previous-release.apk' .github/workflows/release.yml \
        && grep -q -- '--expected-cert "$expected_cert"' .github/workflows/release.yml; then
    ok "CI checks assembled Android identity, signer, and upgrade"
else
    bad "CI checks assembled Android identity, signer, and upgrade"
fi
if grep -q 'script: bash ./scripts/release/android-release-emulator-legs.sh' .github/workflows/release.yml \
        && grep -q 'script: bash ./scripts/release/android-release-emulator-legs.sh' .github/workflows/android-emulator.yml; then
    ok "reactivecircus release legs enter through bash"
else
    bad "reactivecircus release legs enter through bash"
fi
if [[ "$CONF_V" == "../package.json" ]]; then
    ok "tauri.conf.json resolves its version from ../package.json"
else
    bad "tauri.conf.json resolves its version from ../package.json" \
        "version=${CONF_V:-<unset>} — Android metadata rejects a different release path"
fi

MACOS_MIN="$(python3 -c 'import json;print(json.load(open("crates/copypaste-ui/src-tauri/tauri.macos.conf.json"))["bundle"]["macOS"]["minimumSystemVersion"])' 2>/dev/null || true)"
if [[ "$MACOS_MIN" == "14.0" ]] && grep -Fq 'depends_on macos: ">= :sonoma"' Casks/copypaste.rb \
        && grep -Fq '**macOS 14 Sonoma or later** (Apple Silicon):' packaging/release-notes.md \
        && grep -Fq 'runs-on: macos-14' .github/workflows/ci.yml \
        && grep -Fq 'runs-on: macos-14' .github/workflows/release.yml; then
    ok "Tauri, Homebrew, docs, and CI require macOS 14 Sonoma"
else
    bad "macOS support floor is aligned" \
        "expected Tauri 14.0, Homebrew :sonoma, docs naming Sonoma, and macos-14 CI/release jobs; Tauri=${MACOS_MIN:-<unset>}"
fi

group "Android scaffold is checked in, not regenerated"
# gen/android is tracked source (see its README). Two files under it are
# git-ignored on purpose because tauri-build writes them during the Rust build
# with absolute paths into the cargo registry; committing those would pin one
# machine's home directory into the build.
GEN="crates/copypaste-ui/src-tauri/gen/android"
for f in settings.gradle build.gradle.kts gradlew app/build.gradle.kts \
         app/src/main/AndroidManifest.xml \
         app/src/main/res/xml/backup_rules.xml \
         app/src/main/res/xml/data_extraction_rules.xml \
         app/src/main/aidl/android/content/IOnPrimaryClipChangedListener.aidl \
         buildSrc/src/main/java/com/copypaste/app/kotlin/BuildTask.kt; do
    if git ls-files --error-unmatch "$GEN/$f" >/dev/null 2>&1; then
        ok "$f is committed"
    else
        bad "$f is committed" "Gradle reads it at configuration time; it cannot be generated later"
    fi
done
# The capture feature. An APK that builds, installs and captures nothing is a
# worse outcome than a failed build, and it is invisible in the artefact.
for k in CapturePlugin IntakeActivity CaptureTileService ShizukuClipboard \
         ClipListener CaptureService CaptureNotifications ClipQueue; do
    if git ls-files --error-unmatch "$GEN/app/src/main/java/com/copypaste/app/$k.kt" >/dev/null 2>&1; then
        ok "$k.kt is committed"
    else
        bad "$k.kt is committed" "the Android capture feature would build empty"
    fi
done
for f in tauri.settings.gradle app/tauri.build.gradle.kts; do
    if git ls-files --error-unmatch "$GEN/$f" >/dev/null 2>&1; then
        bad "$f is NOT committed" "tauri-build writes it with absolute registry paths; it must stay ignored"
    else
        ok "$f is not committed (tauri-build writes it during the Rust build)"
    fi
done
# Gradle re-enters the CLI through BuildTask.kt in a subprocess. `tauri android
# init` bakes that entry point from however init was invoked, so a scaffold
# generated by a bare `node .../tauri.js` makes Gradle run `node tauri …` and
# fail on a missing module.
if grep -q 'val executable = """npm"""' "$GEN/buildSrc/src/main/java/com/copypaste/app/kotlin/BuildTask.kt" \
   && grep -q '"run", "--", "tauri", "android", "android-studio-script"' "$GEN/buildSrc/src/main/java/com/copypaste/app/kotlin/BuildTask.kt"; then
    ok "BuildTask.kt re-enters the CLI the same way the release job does"
else
    bad "BuildTask.kt re-enters the CLI the same way the release job does" \
        "expected npm + [run, --, tauri, android, android-studio-script]"
fi
# The emulator job's assertions, on fixtures. This is the whole reason they
# live in a script: --self-test proves each detector reports a crash, an
# unencrypted database and a leaked canary when one is there, so the job cannot
# quietly become a test that passes without proving anything.
check "android-smoke.sh --self-test" ./scripts/release/android-smoke.sh --self-test
# The release leg is a second entry point onto the same detectors. Running its
# --self-test too is what catches the entry point itself being broken.
check "android-smoke-release.sh --self-test" ./scripts/release/android-smoke-release.sh --self-test
check "android native accessibility self-test" ./scripts/release/android-native-accessibility.sh --self-test
# The content-picker scenario uses accessibility labels and DocumentsUI ids.
# Its fixtures keep a missing label from becoming a successful no-op.
check "android-storage-transfer.sh --self-test" ./scripts/release/android-storage-transfer.sh --self-test
check "android-cloud-evidence.sh --self-test" ./scripts/release/android-cloud-evidence.sh --self-test
check "macos-cloud-evidence.sh --self-test" ./scripts/release/macos-cloud-evidence.sh --self-test
check "macos-native-evidence.sh --self-test" ./scripts/release/macos-native-evidence.sh --self-test
check "check-feature-ledger.py --self-test" python3 scripts/check-feature-ledger.py --self-test
# The four surfaces README.md calls unverified, and the same reason again: a
# parcel reader that never finds a canary, or a flag reader that says SECURE
# about everything, would turn the rung assertions into decoration.
check "android-rungs.sh --self-test" ./scripts/release/android-rungs.sh --self-test

# Comment lines are excluded: the workflow explains at length why it does not
# do this, and the explanation is not the thing being checked for.
if grep -v '^[[:space:]]*#' .github/workflows/release.yml | grep -q "android init"; then
    bad "release.yml does not run 'tauri android init'" \
        "init cannot write tauri.settings.gradle — it is not in the CLI's template. 'android build' writes it via the Rust build."
else
    ok "release.yml does not run 'tauri android init'"
fi

group "Workflow wiring"
# Structural checks across all four workflows. Everything here is a mistake that
# only a real run would otherwise report, one round trip at a time: an artifact
# name that does not match its producer, an output nothing declares, a job that
# reads a file no job it depends on wrote.
WIRING="$(mktemp)"
if python3 scripts/release/check-wiring.py > "$WIRING" \
   && python3 scripts/release/check-wiring.py --self-test >> "$WIRING" \
   && python3 scripts/release/check-native-parity-wiring.py >> "$WIRING"
then
    while IFS='|' read -r verdict desc detail; do
        [[ -n "${verdict:-}" ]] || continue
        if [[ "$verdict" == "PASS" ]]; then ok "$desc"; else bad "$desc" "$detail"; fi
    done < "$WIRING"
else
    bad "workflow wiring checks ran" "$(cat "$WIRING")"
fi
rm -f "$WIRING"

group "Both platforms reach one release page"
check "publish depends on macos, Android artifact smoke and packaging" python3 - <<'PY'
import sys, yaml
jobs = yaml.safe_load(open(".github/workflows/release.yml"))["jobs"]
missing = [j for j in ("version", "macos", "android", "android-smoke", "packaging") if j not in jobs]
assert not missing, f"release.yml has no {missing} job"
needs = set(jobs["publish"]["needs"])
for j in ("macos", "android", "android-smoke", "packaging"):
    assert j in needs, f"publish does not depend on {j}"

smoke = jobs["android-smoke"]
assert "android" in smoke["needs"], "android-smoke does not wait for Android artifact"
assert "publish" in str(smoke.get("if", "")), "android-smoke does not run for a publishable release"
download = [s for s in smoke["steps"] if str(s.get("uses", "")).startswith("actions/download-artifact")]
download_names = [step.get("with", {}).get("name") for step in download]
assert download_names.count("android") == 1, \
    "android-smoke does not download the published Android artifact"
assert "android-cloud-evidence-apk" in download_names, \
    "android-smoke does not download the configured cloud evidence APK"
runner = [s for s in smoke["steps"] if "android-emulator-runner" in str(s.get("uses", ""))]
script = str(runner[0].get("with", {}).get("script", "")) if len(runner) == 1 else ""
if "android-release-emulator-legs.sh" in script:
    script += open("scripts/release/android-release-emulator-legs.sh").read()
assert "android-smoke-release.sh" in script, \
    "android-smoke does not run the release smoke harness"
PY
for pattern in 'dist/\*\.dmg' 'dist/\*\.apk' 'dist/\*\.tar\.gz'; do
    if grep -qE "$pattern" .github/workflows/release.yml; then
        ok "the release attaches ${pattern//\\/}"
    else
        bad "the release attaches ${pattern//\\/}"
    fi
done

# ADR-0006 names four secrets. If the workflow and the ADR drift, the ADR is
# the thing people read and the workflow is the thing that runs.
for s in ANDROID_KEYSTORE_BASE64 ANDROID_KEYSTORE_PASSWORD ANDROID_KEY_ALIAS ANDROID_KEY_PASSWORD; do
    if grep -q "$s" .github/workflows/release.yml && grep -q "$s" docs/adr/0006-android-release-signing.md; then
        ok "$s is in both the workflow and ADR-0006"
    else
        bad "$s is in both the workflow and ADR-0006"
    fi
done

# A published Android APK must be signed by the durable release key. Unlike the
# never-published emulator fixture, absence of any secret cannot mint a new key
# and turn a normal update into an uninstall-and-data-loss event.
if grep -q 'Android release signing is fail-closed' .github/workflows/release.yml \
   && ! grep -q 'unstable-key' .github/workflows/release.yml \
   && ! grep -q 'CopyPaste Unstable Key' .github/workflows/release.yml; then
    ok "release.yml rejects missing Android signing secrets without an ephemeral-key fallback"
else
    bad "release.yml rejects missing Android signing secrets without an ephemeral-key fallback" \
        "the signing step must fail before artifact upload, not generate an -unstable-key APK"
fi

# `npm audit` must cover every independently locked Node dependency graph: the
# app bundle, the WebKit e2e harness and the token/build toolchain.
if grep -q 'run: npm audit' .github/workflows/ci.yml \
   && [[ "$(grep -c 'run: npm audit' .github/workflows/browser-webkitgtk.yml)" -ge 2 ]]; then
    ok "CI gates UI, e2e and design npm vulnerabilities"
else
    bad "CI gates UI, e2e and design npm vulnerabilities" \
        "expected npm audit after npm ci for all three lockfiles"
fi
# dependency-review sees only PR diffs. Dependency-Check runs after the Android
# build has resolved the actual Gradle graph, including transitive artifacts,
# on the scheduled emulator workflow as well.
if grep -q 'org.owasp:dependency-check-gradle:12.2.2' crates/copypaste-ui/src-tauri/gen/android/build.gradle.kts \
   && grep -q 'dependencyCheckAggregate' .github/workflows/android-emulator.yml \
   && grep -q 'apiKey = nvdApiKey' crates/copypaste-ui/src-tauri/gen/android/build.gradle.kts \
   && grep -Fq 'NVD_API_KEY: ${{ secrets.NVD_API_KEY }}' .github/workflows/android-emulator.yml \
   && grep -q 'name: android-dependency-check-report' .github/workflows/android-emulator.yml \
   && grep -q 'native-nightly.yml' scripts/release/check-wiring.py; then
    ok "Android workflow audits and retains the resolved Gradle dependency graph"
else
    bad "Android workflow audits and retains the resolved Gradle dependency graph" \
        "expected OWASP Dependency-Check with NVD secret and report wiring"
fi

CERT_ERROR=""
EXPECTED_CERT="$(node scripts/release/android-metadata.mjs --field releaseCertificateSha256 2>/dev/null || true)"
[[ "$EXPECTED_CERT" =~ ^[0-9a-f]{64}$ ]] \
    || CERT_ERROR="Cargo.toml must contain one lowercase Android release certificate SHA-256 fingerprint"

if [[ -z "$CERT_ERROR" ]] \
   && grep -q 'releaseCertificateSha256' .github/workflows/release.yml \
   && grep -q 'APK signer fingerprint does not match Cargo.toml' .github/workflows/release.yml; then
    ok "release.yml pins the Android signing certificate before artifact upload"
else
    bad "release.yml pins the Android signing certificate before artifact upload" \
        "${CERT_ERROR:-expected Cargo metadata and the apksigner comparison to remain wired}"
fi

if grep -q 'cargo install tauri-driver --version 2.0.6 --locked' .github/workflows/browser-webkitgtk.yml; then
    ok "browser workflow pins tauri-driver 2.0.6"
else
    bad "browser workflow pins tauri-driver 2.0.6" \
        "unversioned cargo install makes the browser layer non-reproducible"
fi

# ADR-0001's premise, asserted rather than trusted. If a signing credential is
# ever added to the release workflow, this fails and the ADR has to change
# first.
if grep -qE 'APPLE_CERTIFICATE|APPLE_ID|APPLE_TEAM_ID|APPLE_API_KEY|notarytool|stapler' .github/workflows/release.yml; then
    if grep -qE 'if \[\[ -n .*APPLE_SIGNING_IDENTITY' .github/workflows/release.yml; then
        ok "the only Apple names in release.yml are inside the guard"
    else
        bad "release.yml holds no Apple signing credential" \
            "found an Apple credential or notarisation step; ADR-0001 says this pipeline has none"
    fi
else
    ok "release.yml holds no Apple signing credential"
fi
# The bundler matches on the presence of APPLE_SIGNING_IDENTITY, not on its
# value, so `APPLE_SIGNING_IDENTITY=''` makes it sign with the identity "" and
# ask the keychain for a certificate named nothing. That is how the first real
# run of this pipeline failed. The variable has to be removed, not emptied.
if grep -v '^[[:space:]]*#' scripts/release/build-macos-app.sh \
     | grep -qE "APPLE_SIGNING_IDENTITY=(''|\"\"|[[:space:]])"; then
    bad "build-macos-app.sh removes APPLE_SIGNING_IDENTITY rather than emptying it" \
        "an empty value is still Some(\"\") to the CLI; use 'env -u APPLE_SIGNING_IDENTITY'"
elif grep -q 'env -u APPLE_SIGNING_IDENTITY' scripts/release/build-macos-app.sh; then
    ok "build-macos-app.sh removes APPLE_SIGNING_IDENTITY rather than emptying it"
else
    bad "build-macos-app.sh removes APPLE_SIGNING_IDENTITY rather than emptying it" \
        "no 'env -u APPLE_SIGNING_IDENTITY' before the tauri build"
fi

check "macOS bundle executable lookup self-test" \
    ./scripts/release/macos-bundle-self-test.sh

if grep -q 'an Apple signing credential is present in the environment' .github/workflows/release.yml; then
    ok "release.yml still fails the build if a signing identity appears"
else
    bad "release.yml still fails the build if a signing identity appears" \
        "the guard step was removed; it is deliberate (ADR-0001)"
fi

prerelease_probe() {
    set -euo pipefail
    for VERSION in "2.0.0" "2.0.0-alpha.1"; do
        local prerelease=()
        if [[ "$VERSION" == *-* ]]; then
            prerelease+=(--prerelease)
        fi
        printf '%s %s\n' "$VERSION" "${prerelease[*]-}"
    done
}
if out="$(prerelease_probe 2>&1)" \
   && [[ "$out" == *"2.0.0 "* ]] && [[ "$out" == *"2.0.0-alpha.1 --prerelease"* ]]; then
    ok "the --prerelease argument survives set -e for both version shapes"
else
    bad "the --prerelease argument survives set -e" "$out"
fi

group "SQLCipher's crypto backend (ADR-0007)"
# Every invariant here is invisible in a green build. Vendoring OpenSSL on macOS
# succeeds — it just spends what ADR-0007 declined to spend — and a macOS binary
# linked against the runner's Homebrew libcrypto builds, signs, and mounts.
if grep -q 'bundled-sqlcipher-vendored-openssl' Cargo.toml; then
    bad "root Cargo.toml keeps vendored OpenSSL off the default path" \
        "a feature in [workspace.dependencies] reaches macOS too; ADR-0007 scopes it per target"
else
    ok "root Cargo.toml keeps vendored OpenSSL off the default path"
fi

# It must appear somewhere, and that is a separate check from where. The
# feature is one line in one crate manifest, nothing in the workspace refers to
# it, and losing it is invisible off Android: the host build is unaffected and
# the Android job fails minutes later while compiling sqlite3.c against headers
# the NDK sysroot does not have. Commit 2304fbb5 dropped it exactly that way.
if grep -rql 'bundled-sqlcipher-vendored-openssl' --include=Cargo.toml crates >/dev/null; then
    ok "some crate manifest enables vendored OpenSSL (ADR-0007 part 1)"
else
    bad "some crate manifest enables vendored OpenSSL (ADR-0007 part 1)" \
        "no crates/**/Cargo.toml enables it; the Android and Windows builds have no crypto backend to link SQLCipher against"
fi

# Where the feature does appear it must sit under a cfg for a target that has
# no crypto backend to fall back on — android (no OpenSSL in the NDK sysroot)
# and windows (no system OpenSSL, and libsqlite3-sys panics rather than
# choosing). In a plain [dependencies] it vendors on every target — the same
# mistake, wider blast radius, and resolver v2 is the only reason the scoped
# form works at all. macOS is what this guards: it resolves CommonCrypto only
# while no cfg it matches enables the feature.
misplaced=""
while IFS= read -r f; do
    grep -q 'bundled-sqlcipher-vendored-openssl' "$f" || continue
    awk '
        /^[[:space:]]*\[/ {
            scoped = ($0 ~ /^\[target\.[^]]*(android|windows)[^]]*\.dependencies\]/)
        }
        /bundled-sqlcipher-vendored-openssl/ { if (!scoped) offending = 1 }
        END { exit offending ? 1 : 0 }
    ' "$f" || misplaced="${misplaced} ${f}"
done < <(find crates -name Cargo.toml)
if [[ -n "$misplaced" ]]; then
    bad "vendored OpenSSL is scoped to the targets that need it" \
        "found outside an android/windows cfg section in:${misplaced}"
else
    ok "vendored OpenSSL is scoped to the targets that need it"
fi

if grep -q 'name = "openssl-sys", wrappers = \["libsqlite3-sys"\]' deny.toml; then
    ok "deny.toml admits openssl-sys only beneath libsqlite3-sys"
else
    bad "deny.toml admits openssl-sys only beneath libsqlite3-sys" \
        "unscoped, either the Android graph fails cargo-deny or native-tls stops being caught"
fi

# The Android half of the same ADR. cc-rs reads RANLIB_<triple>; a misspelled
# name does not fail, it is simply never read, and the build goes back to the
# `<triple>-ranlib` fallback and dies inside OpenSSL's install_dev. So the names
# are asserted against a fake NDK rather than read.
NDKDIR="$(mktemp -d)"
NDKBIN="$NDKDIR/toolchains/llvm/prebuilt/linux-x86_64/bin"
mkdir -p "$NDKBIN"
printf '#!/usr/bin/env bash\nexit 0\n' > "$NDKBIN/llvm-ar"
printf '#!/usr/bin/env bash\nexit 0\n' > "$NDKBIN/llvm-ranlib"
chmod +x "$NDKBIN/llvm-ar" "$NDKBIN/llvm-ranlib"
if NDK_ENV="$(./scripts/release/android-ndk-env.sh "$NDKDIR" 2>&1)"; then
    absent=""
    for triple in aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android; do
        t="${triple//-/_}"
        grep -qxF "AR_${t}=${NDKBIN}/llvm-ar"         <<<"$NDK_ENV" || absent="${absent} AR_${t}"
        grep -qxF "RANLIB_${t}=${NDKBIN}/llvm-ranlib" <<<"$NDK_ENV" || absent="${absent} RANLIB_${t}"
    done
    if [[ -z "$absent" ]]; then
        ok "android-ndk-env.sh names AR_<triple> and RANLIB_<triple> for all four ABIs"
    else
        bad "android-ndk-env.sh names AR_<triple> and RANLIB_<triple> for all four ABIs" \
            "not emitted, or not pointing at the NDK's llvm tools:${absent}"
    fi
    if [[ "$(grep -c . <<<"$NDK_ENV")" == "8" ]]; then
        ok "android-ndk-env.sh emits those eight lines and nothing else"
    else
        bad "android-ndk-env.sh emits those eight lines and nothing else" \
            "anything else in the output would be appended to GITHUB_ENV verbatim: $NDK_ENV"
    fi
else
    bad "android-ndk-env.sh runs against an NDK layout" "$NDK_ENV"
fi
rm -f "$NDKBIN/llvm-ranlib"
reject "android-ndk-env.sh fails when llvm-ranlib is absent" ./scripts/release/android-ndk-env.sh "$NDKDIR"
reject "android-ndk-env.sh fails when the NDK is absent"     ./scripts/release/android-ndk-env.sh "$NDKDIR/nope"
rm -rf "$NDKDIR"

if grep -q 'android-link-abis.sh' .github/workflows/android-emulator.yml; then
    ok "android-emulator.yml links every ABI the release links"
else
    bad "android-emulator.yml links every ABI the release links" \
        "only release.yml builds non-emulator ABIs, and only on a tag (ADR-0017)"
fi

if grep -q 'no-undefined' scripts/release/android-link-abis.sh; then
    ok "android-link-abis.sh rejects unresolved symbols"
else
    bad "android-link-abis.sh rejects unresolved symbols" \
        "an unresolved symbol otherwise becomes a .so that fails at dlopen"
fi

LINK_TRIPLES="$(sed -n 's/^TRIPLES=(\([^)]*\)).*/\1/p' scripts/release/android-link-abis.sh)"
NDKENV_TRIPLES="$(sed -n 's/^TRIPLES=(\([^)]*\)).*/\1/p' scripts/release/android-ndk-env.sh)"
if [[ -n "$LINK_TRIPLES" && "$LINK_TRIPLES" == "$NDKENV_TRIPLES" ]]; then
    ok "android-link-abis.sh links every ABI android-ndk-env.sh names"
else
    bad "android-link-abis.sh links every ABI android-ndk-env.sh names" \
        "link-abis has [$LINK_TRIPLES], ndk-env has [$NDKENV_TRIPLES]"
fi

reject "android-link-abis.sh fails when NDK_HOME is unset" \
    env -u NDK_HOME ./scripts/release/android-link-abis.sh
reject "android-link-abis.sh fails when NDK_HOME is not a directory" \
    env NDK_HOME=/nonexistent ./scripts/release/android-link-abis.sh

if grep -q 'check-macos-linkage.sh' .github/workflows/release.yml; then
    ok "release.yml runs the macOS linkage check"
else
    bad "release.yml runs the macOS linkage check" \
        "nothing else in this pipeline would notice a bundle linked against the runner's Homebrew"
fi

group "The bits that need real hardware"
cat <<'EOS'
  note  Not checked here, and not checkable here:
          - codesign, spctl, PlistBuddy, hdiutil, xattr, security
          - whether the Tauri macOS bundler emits a bundle at all
          - whether CFBundleShortVersionString keeps an -alpha.1 suffix
            (build-macos-app.sh now hard-fails on a different numeric core and
             warns loudly on a dropped suffix, so the first run answers it)
          - whether a quarantined ad-hoc bundle launches without the hardened
            runtime
          - whether TCC accepts an untrusted self-signed certificate
            (ADR-0001 carries the ten-minute procedure and a table to fill in)
          - the Android build itself: the NDK, Gradle, the Tauri Android
            bundler, and zipalign/apksigner
          - the configured arm64 hardware gate; only its labelled runner and
            attached physical device can execute it
EOS

printf '\n%s\n' "-----------------------------------------------"
printf 'passed %d, failed %d\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]] || exit 1
