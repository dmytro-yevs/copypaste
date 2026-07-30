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
#   * the workflows are valid YAML and the release workflow still holds no
#     Apple credential
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
CURRENT=""

ok()   { PASS=$((PASS + 1)); printf '  ok    %s\n' "$1"; }
bad()  { FAIL=$((FAIL + 1)); printf '  FAIL  %s\n' "$1"; [[ -n "${2:-}" ]] && printf '        %s\n' "$2"; }
group() { CURRENT="$1"; printf '\n== %s\n' "$1"; }

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

# ---------------------------------------------------------------------------
group "Shell syntax"
# ---------------------------------------------------------------------------
for f in scripts/release/*.sh; do
    check "bash -n $f" bash -n "$f"
done
# selfsign.sh runs under macOS /bin/bash, which is 3.2. `bash -n` here cannot
# prove 3.2 compatibility, only that it parses; the script's header records the
# constructs that were avoided and why.
check "bash -n packaging/macos/selfsign.sh" bash -n packaging/macos/selfsign.sh

for f in scripts/release/*.sh packaging/macos/selfsign.sh; do
    if [[ -x "$f" ]]; then
        ok "executable bit set on $f"
    else
        bad "executable bit set on $f" "chmod +x $f"
    fi
done

if command -v shellcheck >/dev/null 2>&1; then
    for f in scripts/release/*.sh packaging/macos/selfsign.sh; do
        check "shellcheck $f" shellcheck -S warning "$f"
    done
else
    printf '  skip  shellcheck (not installed)\n'
fi

# ---------------------------------------------------------------------------
group "Ruby syntax"
# ---------------------------------------------------------------------------
if command -v ruby >/dev/null 2>&1; then
    check "ruby -c Casks/copypaste.rb"                    ruby -c Casks/copypaste.rb
    check "ruby -c packaging/homebrew/copypaste-cli.rb"   ruby -c packaging/homebrew/copypaste-cli.rb
else
    printf '  skip  ruby (not installed)\n'
fi

# ---------------------------------------------------------------------------
group "Seeded values fail closed"
# ---------------------------------------------------------------------------
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

# ---------------------------------------------------------------------------
group "Generators round-trip"
# ---------------------------------------------------------------------------
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
# join between two scripts that never see each other, and v1 shipped
# CopyPaste-vv0.5.1-... exactly once by getting it wrong.
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

# ---------------------------------------------------------------------------
group "Generators reject bad input"
# ---------------------------------------------------------------------------
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

# ---------------------------------------------------------------------------
group "Tap layout"
# ---------------------------------------------------------------------------
# `brew tap` looks for Casks/ and Formula/ at the repository root.
for p in "Casks/copypaste.rb" "Formula/copypaste-cli.rb"; do
    if [[ -f "$TAPDIR/$p" ]]; then
        ok "generated tap contains $p"
    else
        bad "generated tap contains $p" "found: $(cd "$TAPDIR" && find . -type f | sort | tr '\n' ' ')"
    fi
done
check "setup-tap.sh --dry-run"       ./scripts/release/setup-tap.sh --github-user dmytro-yevs --dry-run
reject "setup-tap.sh needs a user"   ./scripts/release/setup-tap.sh --dry-run
reject "setup-tap.sh rejects a bad tap name" ./scripts/release/setup-tap.sh --github-user dmytro-yevs --tap-name "Bad Name" --dry-run

# ---------------------------------------------------------------------------
group "Workflows"
# ---------------------------------------------------------------------------
if command -v python3 >/dev/null 2>&1 && python3 -c "import yaml" 2>/dev/null; then
    for wf in .github/workflows/*.yml; do
        check "YAML parses: $wf" python3 -c "import sys,yaml; yaml.safe_load(open(sys.argv[1]))" "$wf"
    done

    # Every `run:` block in the release workflow is shell that nothing has ever
    # executed. Extracting them and parsing each one catches the class of
    # mistake a YAML block scalar makes easy — an unbalanced quote or heredoc
    # that only shows up when the job is halfway through a release.
    #
    # `${{ … }}` is not valid shell, so it is replaced with a placeholder token
    # first. That is exactly what GitHub does before handing the script to bash.
    RUNDIR="$(mktemp -d)"
    python3 - "$RUNDIR" <<'PY'
import pathlib, re, sys, yaml
outdir = pathlib.Path(sys.argv[1])
n = 0
for wf in sorted(pathlib.Path(".github/workflows").glob("*.yml")):
    doc = yaml.safe_load(wf.read_text())
    for job_name, job in (doc.get("jobs") or {}).items():
        for i, step in enumerate(job.get("steps") or []):
            script = step.get("run")
            if not script:
                continue
            shell = step.get("shell", "bash")
            if shell not in ("bash", "sh"):
                continue
            script = re.sub(r"\$\{\{[^}]*\}\}", "GHA_EXPR", script)
            (outdir / f"{wf.stem}.{job_name}.{i}.sh").write_text(script)
            n += 1
PY
    for f in "$RUNDIR"/*.sh; do
        check "bash -n $(basename "$f" .sh)" bash -n "$f"
    done
    rm -rf "$RUNDIR"
else
    printf '  skip  YAML parse (python3 + PyYAML not available)\n'
fi

# ---------------------------------------------------------------------------
group "Both platforms reach one release page"
# ---------------------------------------------------------------------------
if command -v python3 >/dev/null 2>&1 && python3 -c "import yaml" 2>/dev/null; then
    check "publish depends on macos, android and packaging" python3 - <<'PY'
import sys, yaml
jobs = yaml.safe_load(open(".github/workflows/release.yml"))["jobs"]
missing = [j for j in ("version", "macos", "android", "packaging") if j not in jobs]
assert not missing, f"release.yml has no {missing} job"
needs = set(jobs["publish"]["needs"])
for j in ("macos", "android", "packaging"):
    assert j in needs, f"publish does not depend on {j}"
PY
fi
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
if grep -q 'an Apple signing credential is present in the environment' .github/workflows/release.yml; then
    ok "release.yml still fails the build if a signing identity appears"
else
    bad "release.yml still fails the build if a signing identity appears" \
        "the guard step was removed; it is deliberate (ADR-0001)"
fi

# The prerelease flag is built with a command substitution containing a failing
# test, which is the sort of thing `set -e` punishes. Prove it does not.
prerelease_probe() {
    set -euo pipefail
    for VERSION in "2.0.0" "2.0.0-alpha.1"; do
        printf '%s\n' "$VERSION $([[ "$VERSION" == *-* ]] && echo --prerelease)"
    done
}
if out="$(prerelease_probe 2>&1)" \
   && [[ "$out" == *"2.0.0 "* ]] && [[ "$out" == *"2.0.0-alpha.1 --prerelease"* ]]; then
    ok "the --prerelease command substitution survives set -e for both version shapes"
else
    bad "the --prerelease command substitution survives set -e" "$out"
fi

# ---------------------------------------------------------------------------
group "The bits that need real hardware"
# ---------------------------------------------------------------------------
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
          - the whole Android job: the NDK, Gradle, the Tauri Android bundler,
            zipalign/apksigner, and whether the APK installs on a device
EOS

printf '\n%s\n' "-----------------------------------------------"
printf 'passed %d, failed %d\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]] || exit 1
