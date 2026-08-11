#!/usr/bin/env python3
# check-wiring.py — structural checks over .github/workflows/, for check.sh.
#
# Prints one `PASS|description|` or `FAIL|description|detail` line per check and
# always exits 0; check.sh counts them. Run from the repository root.
#
# Everything here is a mistake that only a real run would otherwise report, one
# round trip at a time: an artifact name that does not match its producer, an
# output nothing declares, a job that reads a file no job it depends on wrote.
import json, pathlib, re, sys, yaml

SELF_TEST = "--self-test" in sys.argv

WF = pathlib.Path(".github/workflows")


def emit(cond, desc, detail=""):
    print("{}|{}|{}".format("PASS" if cond else "FAIL", desc, "" if cond else detail))


def rec(cond, desc, detail=""):
    # --self-test reports only its own verdicts, so check.sh counts two runs
    # rather than every check in this file twice.
    if not SELF_TEST:
        emit(cond, desc, detail)


def steps(job):
    return job.get("steps") or []


def as_list(v):
    return [v] if isinstance(v, str) else (v or [])


def closure(jobs, name):
    seen, stack = set(), list(as_list(jobs.get(name, {}).get("needs")))
    while stack:
        n = stack.pop()
        if n in seen:
            continue
        seen.add(n)
        stack += as_list(jobs.get(n, {}).get("needs"))
    return seen


def min_major(rng):
    lows = []
    for alt in rng.split("||"):
        match = re.search(r"(?:>=?|\^|~)?\s*(\d+)", alt)
        if match:
            lows.append(int(match.group(1)))
    return min(lows) if lows else 0


docs = {p.name: yaml.safe_load(p.read_text()) for p in sorted(WF.glob("*.yml"))}
text = {p.name: p.read_text() for p in sorted(WF.glob("*.yml"))}


release_jobs = docs["release.yml"].get("jobs") or {}
gate = release_jobs.get("supabase-gate") or {}
gate_body = "\n".join(step.get("run") or "" for step in steps(gate))
rec("real-supabase.sh" in gate_body,
    "release.yml runs the disposable real-Supabase gate")
rec("supabase-gate" in closure(release_jobs, "publish"),
    "release.yml blocks publish on the real-Supabase gate")

macos_smoke = pathlib.Path("scripts/release/smoke-macos-dmg.sh").read_text()
rec("macos-native-evidence.sh artifacts/release-macos-native" in macos_smoke,
    "macOS smoke captures native evidence before removing the installed app")
rec("macos-cloud-evidence.sh artifacts/release-macos-cloud" in macos_smoke,
    "macOS smoke captures the cloud account lifecycle from the installed app")

ci_jobs = docs["ci.yml"].get("jobs") or {}
documentation = ci_jobs.get("documentation") or {}
documentation_body = "\n".join(step.get("run") or "" for step in steps(documentation))
rec("check-docs.py" in documentation_body,
    "ci.yml gates documentation links and unfinished-work markers")


# --- artifacts: names match, and the consumer depends on the producer --------
for wf, doc in docs.items():
    jobs = doc.get("jobs") or {}
    produced = {}
    for jn, j in jobs.items():
        for s in steps(j):
            if (s.get("uses") or "").startswith("actions/upload-artifact"):
                produced.setdefault((s.get("with") or {}).get("name", "artifact"), set()).add(jn)
    for jn, j in jobs.items():
        deps = closure(jobs, jn)
        for s in steps(j):
            if not (s.get("uses") or "").startswith("actions/download-artifact"):
                continue
            nm = (s.get("with") or {}).get("name")
            if nm is None:
                continue
            prod = produced.get(nm)
            rec(bool(prod), "{}: {} downloads artifact '{}' that a job uploads".format(wf, jn, nm),
                "no upload-artifact step is named '{}'; uploaded names are {}".format(nm, sorted(produced)))
            if prod:
                rec(bool(prod & deps), "{}: {} needs the job producing '{}'".format(wf, jn, nm),
                    "'{}' comes from {} but {} needs {}".format(nm, sorted(prod), jn, sorted(deps)))

# --- outputs: declared on the job, and written to $GITHUB_OUTPUT ------------
for wf, doc in docs.items():
    jobs = doc.get("jobs") or {}
    for jn, key in sorted(set(re.findall(r"needs\.([A-Za-z0-9_-]+)\.outputs\.([A-Za-z0-9_-]+)", text[wf]))):
        declared = (jobs.get(jn) or {}).get("outputs") or {}
        rec(key in declared, "{}: needs.{}.outputs.{} is declared".format(wf, jn, key),
            "job '{}' declares {}".format(jn, sorted(declared)))
        m = re.search(r"steps\.([A-Za-z0-9_-]+)\.outputs\.([A-Za-z0-9_-]+)", str(declared.get(key, "")))
        if not m:
            continue
        sid, skey = m.groups()
        body = "".join(s.get("run") or "" for s in steps(jobs[jn]) if s.get("id") == sid)
        rec(re.search(r"{}=.*>>[^\n]*GITHUB_OUTPUT".format(re.escape(skey)), body) is not None,
            "{}: step '{}' writes {} to GITHUB_OUTPUT".format(wf, sid, skey),
            "no line in step '{}' assigns {} into GITHUB_OUTPUT".format(sid, skey))

# --- permissions ------------------------------------------------------------
for wf, doc in docs.items():
    rec((doc.get("permissions") or {}) == {"contents": "read"},
        "{}: workflow permissions are contents read".format(wf), repr(doc.get("permissions")))
    writers = sorted(jn for jn, j in (doc.get("jobs") or {}).items() if "write" in str(j.get("permissions") or ""))
    if wf == "release.yml":
        rec(writers == ["publish"], "release.yml: only publish widens permissions",
            "jobs holding a write permission: {}".format(writers))
    else:
        rec(not writers, "{}: no job widens permissions".format(wf),
            "jobs holding a write permission: {}".format(writers))

# --- `find … | head` under pipefail ----------------------------------------
# head exits after the first line, find dies of SIGPIPE, pipefail makes the
# pipeline 141 and set -e kills the step with no message at all.
for wf, doc in docs.items():
    for jn, j in (doc.get("jobs") or {}).items():
        for i, s in enumerate(steps(j)):
            body = s.get("run") or ""
            if "pipefail" not in body:
                continue
            hits = [l.strip() for l in body.splitlines() if re.search(r"\bfind\b.*\|\s*head\b", l)]
            rec(not hits, "{}: {} step {} has no 'find piped into head' under pipefail".format(wf, jn, i),
                "; ".join(hits) + "  — use find -print -quit")

# --- runner images ----------------------------------------------------------
# One table per OS family, so a new platform is a line of data rather than a
# second copy of the loop below. The flag marks a family whose jobs must all
# agree on one image: what the artefact is built against must not move without
# a commit. Linux jobs are free to differ, and several do.
RUNNER_IMAGES = {
    "Linux": (False, {"ubuntu-latest", "ubuntu-24.04", "ubuntu-22.04"}),
    "macOS": (True, {"macos-14", "macos-15", "macos-latest"}),
    "Windows": (True, {"windows-2022", "windows-2025", "windows-latest"}),
}
known = {label for _, labels in RUNNER_IMAGES.values() for label in labels}


def runner_image_checks(workflows):
    used = {}
    for wf, doc in workflows.items():
        for jn, j in (doc.get("jobs") or {}).items():
            r = j.get("runs-on")
            if not isinstance(r, str):
                continue
            # A job that fans out over `matrix.runner` is spreading itself
            # across images on purpose, so its labels are validated but left
            # out of the one-image rule below, which is about what a shipped
            # artefact is built against.
            if r == "${{ matrix.runner }}":
                runners = ((j.get("strategy") or {}).get("matrix") or {}).get("runner") or []
                yield (bool(runners) and set(runners) <= known,
                       "{}: {} matrix uses known runner images".format(wf, jn),
                       repr(runners))
                continue
            family = next((f for f, (_, labels) in RUNNER_IMAGES.items() if r in labels), None)
            yield (family is not None,
                   "{}: {} runs on a known image ({})".format(wf, jn, r),
                   "unrecognised runner label")
            if family:
                used.setdefault(family, set()).add(r)
    for family, (one_image, _) in RUNNER_IMAGES.items():
        if not one_image:
            continue
        images = sorted(used.get(family, ()))
        yield (len(images) <= 1,
               "every {} job uses the same runner image {}".format(family, images),
               "mixed {} runners: {}".format(family, images))


for check in runner_image_checks(docs):
    rec(*check)

# --- one ref per action, across every workflow ------------------------------
refs = {}
for wf, doc in docs.items():
    for jn, j in (doc.get("jobs") or {}).items():
        for s in steps(j):
            u = s.get("uses")
            if u and "@" in u:
                a, r = u.rsplit("@", 1)
                refs.setdefault(a, set()).add(r)
for a, rs in sorted(refs.items()):
    rec(len(rs) == 1, "{} is pinned to one ref everywhere".format(a), "refs in use: {}".format(sorted(rs)))

# --- dependency review policy ----------------------------------------------
supply = docs.get("supply-chain.yml") or {}
review_job = (supply.get("jobs") or {}).get("dependency-review") or {}
review_steps = [s for s in steps(review_job)
                if str(s.get("uses") or "").split("@", 1)[0] == "actions/dependency-review-action"]
rec(len(review_steps) == 1, "supply-chain.yml: dependency-review-action runs exactly once",
    "found {} matching steps".format(len(review_steps)))
for review in review_steps:
    use = str(review.get("uses") or "")
    ref = use.rsplit("@", 1)[1] if "@" in use else ""
    rec(re.fullmatch(r"[0-9a-f]{40}", ref) is not None,
        "supply-chain.yml: dependency-review-action is pinned to a full commit SHA", repr(use))
    severity = str((review.get("with") or {}).get("fail-on-severity", ""))
    rec(severity == "low", "supply-chain.yml: dependency review fails on low severity",
        "fail-on-severity is {!r}".format(severity or None))

# --- the Node a setup-node job actually gets ---------------------------------
# npm treats an unmet `engines.node` as a warning unless engine-strict is set,
# so a lockfile can outgrow the runners without anything failing — until the day
# a dependency uses the syntax it asked for. @zxing/library 0.23.0 raised the
# floor to 24 while every job pinned 22.
locks = {}
for wf, doc in docs.items():
    for jn, j in (doc.get("jobs") or {}).items():
        for s in steps(j):
            if not (s.get("uses") or "").startswith("actions/setup-node"):
                continue
            with_ = s.get("with") or {}
            pinned = str(with_.get("node-version", ""))
            m = re.match(r"(\d+)", pinned)
            rec(bool(m), "{}: {} pins a Node major ({})".format(wf, jn, pinned or "<unset>"),
                "setup-node without an explicit node-version follows whatever the runner ships")
            if not m:
                continue
            major = int(m.group(1))
            for lock in str(with_.get("cache-dependency-path", "")).split():
                p = pathlib.Path(lock)
                if not p.is_file():
                    rec(False, "{}: {} caches an existing lockfile ({})".format(wf, jn, lock), "no such file")
                    continue
                if lock not in locks:
                    pkgs = json.loads(p.read_text()).get("packages", {})
                    need = [(k, (v.get("engines") or {}).get("node")) for k, v in pkgs.items()
                            if (v.get("engines") or {}).get("node")]
                    locks[lock] = max(((min_major(e), k) for k, e in need), default=(0, ""))
                floor, who = locks[lock]
                rec(major >= floor,
                    "{}: {} runs Node {} for {}".format(wf, jn, major, lock),
                    "{} declares engines.node needing >= {}".format(who, floor))

# --- the toolchain a bare `cargo` actually resolves to ----------------------
# dtolnay/rust-toolchain sets its toolchain with `rustup default`, and
# rust-toolchain.toml outranks the default. So a job that installs 1.96 and then
# runs a bare `cargo` builds on whatever `stable` is that week — and on any
# target the pinned toolchain installed but stable did not, it fails outright.
for wf, doc in docs.items():
    wenv = doc.get("env") or {}
    for jn, j in (doc.get("jobs") or {}).items():
        if not any((s.get("uses") or "").startswith("dtolnay/rust-toolchain") for s in steps(j)):
            continue
        hits = []
        for s in steps(j):
            for line in (s.get("run") or "").splitlines():
                line = line.strip()
                if re.search(r"(^|[;&|(]\s*|\s)cargo\s+(?!\+)", line) or re.search(r"tauri\s+--\s+\S*\s*build", line):
                    hits.append(line)
                for m in re.findall(r"\./scripts/[\w/.-]+\.sh", line):
                    p = pathlib.Path(m)
                    if p.is_file() and re.search(r"(?m)^\s*cargo\s+(?!\+)", p.read_text()):
                        hits.append("{} runs a bare cargo".format(m))
        if not hits:
            continue
        env = dict(wenv, **(j.get("env") or {}))
        rec("RUSTUP_TOOLCHAIN" in env, "{}: {} pins RUSTUP_TOOLCHAIN for its bare cargo".format(wf, jn),
            "resolves through rust-toolchain.toml instead: {}".format(hits[:3]))

# --- the Android NDK binutils wiring ---------------------------------------
# openssl-src asks cc-rs for AR and RANLIB, and cc-rs falls back to
# `<triple>-ranlib` — a wrapper no NDK has shipped since r23 — unless something
# exports RANLIB_<triple>. android-ndk-env.sh exports it, per triple, so its
# list and the list of targets the job installs have to stay the same set: a
# target it misses fails only after several minutes of OpenSSL build.
android = (docs["release.yml"].get("jobs") or {}).get("android") or {}
wiring = [s for s in steps(android) if "android-ndk-env.sh" in (s.get("run") or "")]
rec(len(wiring) == 1, "release.yml: android runs android-ndk-env.sh exactly once",
    "found {} steps invoking it".format(len(wiring)))
rec(any("GITHUB_ENV" in (s.get("run") or "") for s in wiring),
    "release.yml: android-ndk-env.sh output reaches GITHUB_ENV",
    "the script only prints; nothing reads it unless it is appended to GITHUB_ENV")
installed = set()
for s in steps(android):
    if (s.get("uses") or "").startswith("dtolnay/rust-toolchain"):
        installed |= {t.strip() for t in str((s.get("with") or {}).get("targets", "")).split(",") if t.strip()}
script = pathlib.Path("scripts/release/android-ndk-env.sh")
m = re.search(r"TRIPLES=\(([^)]*)\)", script.read_text()) if script.is_file() else None
listed = set((m.group(1) if m else "").split())
rec(bool(installed) and listed == installed,
    "android-ndk-env.sh covers every Android target the job installs",
    "script has {}, the toolchain step installs {}".format(sorted(listed), sorted(installed)))

# --- Gradle dependency cache ------------------------------------------------
for wf, doc in docs.items():
    for jn, j in (doc.get("jobs") or {}).items():
        if not any("android build" in (s.get("run") or "") for s in steps(j)):
            continue
        java = [s for s in steps(j) if (s.get("uses") or "").startswith("actions/setup-java")]
        rec(len(java) == 1, "{}: {} configures Java exactly once".format(wf, jn),
            "found {} setup-java steps".format(len(java)))
        for setup in java:
            with_ = setup.get("with") or {}
            rec(with_.get("cache") == "gradle",
                "{}: {} caches Gradle dependencies".format(wf, jn),
                "setup-java cache is {!r}".format(with_.get("cache")))
            paths = str(with_.get("cache-dependency-path", ""))
            rec("gen/android" in paths and "gradle-wrapper.properties" in paths,
                "{}: {} keys the Gradle cache from Android build files".format(wf, jn),
                "cache-dependency-path is {!r}".format(paths or None))

# --- shell discipline in the workflows nothing has ever run -----------------
for wf in ("release.yml", "android-emulator.yml"):
    for jn, j in ((docs.get(wf) or {}).get("jobs") or {}).items():
        for i, s in enumerate(steps(j)):
            body = s.get("run")
            # A one-command step gains nothing: GitHub already runs `bash -e`.
            # The rule is for multi-line blocks, where -u and -o pipefail are
            # the ones that matter.
            if not body or len(body.strip().splitlines()) < 2:
                continue
            prefix = '$ErrorActionPreference = "Stop"' if s.get("shell") == "pwsh" else "set -euo pipefail"
            rec(body.lstrip().startswith(prefix),
                "{}: {} step {} opens with {}".format(wf, jn, i, prefix),
                "opens with: {}".format(body.lstrip().splitlines()[0][:60]))

# --- the emulator smoke test ------------------------------------------------
# Nothing here can boot an emulator. What it can do is keep that job from
# becoming one that passes without proving anything: the assertions have to
# live in the script check.sh already parses, shellchecks and self-tests; the
# action has to be allowed to fail the job; and the APK it is handed has to be
# the shape the script's run-as assertions need.
emu = docs.get("android-emulator.yml")
rec(emu is not None, "android-emulator.yml exists",
    "it is the only thing in this repository that runs a line of Kotlin")
if emu:
    ejobs = emu.get("jobs") or {}
    apk_steps = steps(ejobs.get("apk") or {})
    dependency_audit = next(
        (step for step in apk_steps if "dependencyCheckAggregate" in (step.get("run") or "")),
        {},
    )
    audit_report = next(
        (step for step in apk_steps
         if (step.get("with") or {}).get("name") == "android-dependency-check-report"),
        {},
    )
    rec((dependency_audit.get("env") or {}).get("NVD_API_KEY") == "${{ secrets.NVD_API_KEY }}",
        "Android dependency audit reads the NVD API key secret",
        "forks may leave the secret empty, but repository runs must pass it to Gradle")
    rec(audit_report.get("if") == "always()",
        "Android dependency audit report uploads after a failed gate",
        "the CVE report is evidence for the failure and must not be skipped with later steps")
    rec((audit_report.get("with") or {}).get("if-no-files-found") == "ignore",
        "Android dependency report tolerates failures before report generation",
        "always() must not hide the original audit failure when no report exists")
    triggers = emu.get(True) or emu.get("on") or {}
    # Reversed deliberately. It used to assert the opposite — ten minutes of
    # runner time was judged too much per merge — but this is Android's only
    # authoritative layer (docs/rewrite/testing-policy.md), and a layer that
    # never gates a merge cannot catch the merge that breaks it. Paths keep the
    # cost off the changes that cannot reach Android.
    rec({"push", "pull_request"} <= set(triggers),
        "android-emulator.yml gates pushes and pull requests",
        "Android's authoritative layer has to run before the merge it would fail")
    for event in ("push", "pull_request"):
        paths = (triggers.get(event) or {}).get("paths") or []
        rec(any(p.startswith("crates/copypaste-ui/src/") for p in paths)
            and any(p.startswith("crates/copypaste-ipc") for p in paths),
            f"android-emulator.yml {event} filter covers the shared frontend and the wire contract",
            "a path filter that omits them hides cross-platform breakage")
    nightly_jobs = (docs.get("native-nightly.yml") or {}).get("jobs") or {}
    android_matrix = (nightly_jobs.get("android") or {}).get("strategy", {}).get("matrix", {})
    rec("workflow_call" in triggers and "workflow_dispatch" in triggers
        and set(android_matrix.get("api-level") or []) == {34, 36},
        "Android runs on a nightly API matrix and on demand",
        "expected reusable workflow plus nightly API 34/36 matrix")

    emulator_matrix = ((ejobs.get("emulator") or {}).get("strategy") or {}).get("matrix") or {}
    api_matrix = str(emulator_matrix.get("api-level", ""))
    rec("[24,29,33,34,36]" in api_matrix,
        "android-emulator.yml schedules the representative API matrix",
        "expected 24, 29, 33, 34 and 36 in the scheduled matrix: {!r}".format(api_matrix))

    # Both legs, and the build flag that separates them. The debug leg must
    # stay debuggable or it loses run-as and every filesystem assertion with
    # it; the release leg must stay *not* debuggable or R8 never runs and it
    # becomes a slower copy of the debug one.
    for emulator_job, apk_job, script, debug in (
        ("emulator", "apk", "android-smoke.sh", True),
        ("release-emulator", "release-apk", "android-smoke-release.sh", False),
    ):
        ejob = ejobs.get(emulator_job) or {}
        rec(bool(ejob), "android-emulator.yml has a {} job".format(emulator_job),
            "jobs present: {}".format(sorted(ejobs)))
        runners = [s for s in steps(ejob) if (s.get("uses") or "").startswith("reactivecircus/android-emulator-runner")]
        rec(len(runners) == 1, "{}: exactly one emulator-runner step".format(emulator_job),
            "found {} — AVD management is the action's job, not this file's".format(len(runners)))
        for s in runners:
            with_ = s.get("with") or {}
            runner_script = str(with_.get("script", ""))
            runner_sources = [runner_script]
            for candidate in re.findall(r"scripts/release/([A-Za-z0-9._-]+\.sh)", runner_script):
                candidate_path = pathlib.Path("scripts") / "release" / candidate
                if candidate_path.exists():
                    runner_sources.append(candidate_path.read_text(encoding="utf-8"))
            runner_source = "\n".join(runner_sources)
            rec(script in runner_source,
                "{} runs scripts/release/{}".format(emulator_job, script),
                "assertions belong in a script check.sh can parse and self-test, not in YAML: {!r}".format(with_.get("script")))
            rec("android-storage-transfer.sh" in runner_source,
                "{} runs the native storage transfer scenario".format(emulator_job),
                "export/import through DocumentsUI must gate both Android build types")
            if debug:
                rec("TRANSFER_REQUIRE_RUN_AS=1" in runner_source,
                    "{} requires ciphertext inspection".format(emulator_job),
                    "the debuggable leg must fail if it cannot read and inspect the SQLCipher files")
            rec(not s.get("continue-on-error"), "{}: the emulator step can fail the job".format(emulator_job),
                "continue-on-error would make the whole run decorative")
            rec(str(with_.get("arch")) == "x86_64", "{}: the AVD is x86_64".format(emulator_job), repr(with_.get("arch")))
            before = steps(ejob)[:steps(ejob).index(s)]
            rec(any("99-kvm4all" in (p.get("run") or "") for p in before),
                "{}: KVM is enabled before the emulator starts".format(emulator_job),
                "without the udev rule the AVD boots unaccelerated, which reads as a hang")

        # The invocation, not the prose around it: the comments there name the
        # flags, so a check over the whole block would pass on a build that had
        # dropped them.
        build = [l.strip() for s in steps(ejobs.get(apk_job) or {})
                 for l in (s.get("run") or "").splitlines()
                 if "android build" in l and not l.strip().startswith("#")]
        rec(bool(build), "{}: builds an APK".format(apk_job), "no `android build` line in it")
        rec(any(("--debug" in l) == debug for l in build) and all(("--debug" in l) == debug for l in build),
            "{}: builds {} APK".format(apk_job, "a debug" if debug else "the minified release"),
            "--debug is {} here: {}".format("required" if debug else "what makes R8 not run", build))
        rec(all("--target x86_64" in l for l in build), "{}: builds for the AVD's ABI".format(apk_job),
            "an APK with no x86_64 native library installs and then dies on load: {}".format(build))

    for emulator_job in ("emulator", "release-emulator", "release-api33-smoke"):
        body = "\n".join(s.get("run") or "" for s in steps(ejobs.get(emulator_job) or {}))
        rec("pip install --requirement requirements-ci.txt" in body,
            "{} installs the PNG decoder".format(emulator_job),
            "the screenshot gate imports Pillow before the emulator runner starts")

release = docs.get("release.yml") or {}
release_jobs = release.get("jobs") or {}
hardware = release_jobs.get("android-hardware") or {}
rec(bool(hardware), "release.yml has a physical arm64 Android gate",
    "an x86_64 emulator cannot execute the native library users install")
if hardware:
    labels = hardware.get("runs-on") or []
    labels = labels if isinstance(labels, list) else [labels]
    rec({"self-hosted", "ARM64", "android-device"} <= set(labels),
        "the Android hardware gate selects the arm64 device runner", repr(labels))
    publish_needs = (release_jobs.get("publish") or {}).get("needs") or []
    publish_needs = publish_needs if isinstance(publish_needs, list) else [publish_needs]
    rec("android-hardware" in publish_needs,
        "publishing requires the Android hardware gate", repr(publish_needs))
    bodies = "\n".join(s.get("run") or "" for s in steps(hardware))
    rec("arm64-v8a" in bodies and "android-smoke-release.sh" in bodies
        and "android-storage-transfer.sh" in bodies,
        "the hardware gate verifies arm64 and runs release smoke plus storage transfer",
        "the gate must reject another ABI and exercise the signed APK through DocumentsUI")

release_smoke = release_jobs.get("android-smoke") or {}
release_runner_scripts = [str((step.get("with") or {}).get("script", ""))
                          for step in steps(release_smoke)
                          if (step.get("uses") or "").startswith("reactivecircus/android-emulator-runner")]
release_script = release_runner_scripts[0] if len(release_runner_scripts) == 1 else ""
if "android-release-emulator-legs.sh" in release_script:
    release_script += "\n" + pathlib.Path("scripts/release/android-release-emulator-legs.sh").read_text()
release_env = next((step.get("env") or {} for step in steps(release_smoke)
                    if (step.get("uses") or "").startswith("reactivecircus/android-emulator-runner")), {})
release_android = release_jobs.get("android") or {}
release_android_uploads = [str((step.get("with") or {}).get("name", ""))
                           for step in steps(release_android)
                           if (step.get("uses") or "").startswith("actions/upload-artifact")]
release_smoke_downloads = [str((step.get("with") or {}).get("name", ""))
                           for step in steps(release_smoke)
                           if (step.get("uses") or "").startswith("actions/download-artifact")]
release_android_body = "\n".join(step.get("run") or "" for step in steps(release_android))
rec("android-upgrade-fixture" in release_android_uploads
    and "android-upgrade-fixture" in release_smoke_downloads
    and release_env.get("PREVIOUS_APK") == "upgrade-dist/copypaste-previous-release.apk"
    and "--write-overlay" in release_android_body,
    "release.yml tests a separately signed previous-version APK",
    "the publish workflow must build, sign, upload, download and install its own upgrade fixture")
rec("android-storage-transfer.sh" in release_script,
    "release.yml runs storage transfer against the signed Android artifact",
    "the release evidence must come from the APK that publish consumes")
rec("android-cloud-evidence.sh" in release_script and release_env.get("CLOUD_MODE") == "all",
    "release.yml captures configured and unconfigured Android cloud evidence",
    "the signed release APK and configured evidence APK must run in the release emulator gate")
release_uploads = [str((step.get("with") or {}).get("name", ""))
                   for step in steps(release_smoke)
                   if (step.get("uses") or "").startswith("actions/upload-artifact")]
rec("release-android-cloud-evidence" in release_uploads,
    "release.yml uploads dedicated Android cloud evidence",
    repr(release_uploads))

release_android_workflow = text["release.yml"]
emulator_android_workflow = text["android-emulator.yml"]
for name, body in (("release.yml", release_android_workflow),
                   ("android-emulator.yml", emulator_android_workflow)):
    rec("COPYPASTE_CLOUD_URL=http://127.0.0.1:47800" in body
        and "--features cloud-evidence" in body and "10.0.2.2:47800" not in body,
        f"{name} confines plaintext cloud evidence to its loopback feature",
        "configured evidence must use 127.0.0.1 and cloud-evidence, never emulator plaintext routing")
rec(bool(re.search(r"^\s*npm run tauri -- android build --apk\s*$", release_android_workflow, re.M)),
    "release.yml builds the published APK without cloud evidence configuration")
rec(bool(re.search(r"^\s*npm run tauri -- android build --apk --target x86_64\s*$",
                   emulator_android_workflow, re.M)),
    "android-emulator.yml rebuilds the shipped APK without cloud evidence configuration")

ui_cargo = pathlib.Path("crates/copypaste-ui/src-tauri/Cargo.toml").read_text()
embedded_cloud = pathlib.Path("crates/copypaste-ui/src-tauri/src/backend/embedded/cloud.rs").read_text()
android_cloud = pathlib.Path("scripts/release/android-cloud-evidence.sh").read_text()
android_ui_evidence = pathlib.Path("scripts/release/android-ui-evidence-lib.sh").read_text()
android_screencap = pathlib.Path("scripts/release/android-screencap-lib.sh").read_text()
png_evidence = pathlib.Path("scripts/release/png_evidence.py").read_text()
rec('cloud-evidence = ["copypaste-cloud/test-endpoints"]' in ui_cargo
    and 'cfg(feature = "cloud-evidence")' in embedded_cloud
    and "CloudConfig::new_loopback" in embedded_cloud,
    "the UI cloud evidence feature selects the guarded loopback constructor")
rec('adb reverse "tcp:$STUB_PORT" "tcp:$STUB_PORT"' in android_cloud,
    "Android cloud evidence reverses the host stub onto device loopback")
rec("capture_android_png" in android_ui_evidence
    and "adb exec-out screencap" not in android_ui_evidence,
    "Android UI evidence reuses the fail-closed PNG capture helper",
    "storage and cloud evidence must reject zero-byte and malformed screenshots")
rec("bounded_adb -s \"$1\" emu screenrecord screenshot" in android_screencap
    and "run_with_android_screencap_timeout" in android_screencap
    and "bounded_adb get-serialno" in android_screencap
    and "append_bounded_adb_diagnostic" in android_screencap
    and "route=adb-get-serialno" in android_screencap
    and '[[ "$serial" == emulator-* ]]' in android_screencap
    and "Screenshot_*.png" in android_screencap
    and 'for attempt in $(seq 1 "$ANDROID_SCREENCAP_ATTEMPTS")' in android_screencap
    and "transport retries exhausted" in android_screencap
    and "failed PNG evidence validation; not retried" in android_screencap,
    "Android emulator evidence uses bounded host transport retries",
    "guest adb screencap can return zero-byte or black frames while the host framebuffer is painted")
rec("adb exec-out screencap -p" in android_screencap
    and "adb shell screencap -p" in android_screencap
    and "cleanup_status" in android_screencap,
    "physical Android evidence retains bounded adb capture and fail-closed cleanup")
rec("Image.alpha_composite" in png_evidence
    and "MIN_VISIBLE_CONTENT_FRACTION = 0.01" in png_evidence
    and "ImageFilter.BoxBlur(1)" in png_evidence
    and "locally coherent pixels" in png_evidence
    and "near-black-checker" in png_evidence
    and "isolated-noise" in png_evidence
    and "transparent-hidden-rgb" in png_evidence
    and "a decodable black emulator frame is rejected" in android_screencap,
    "native screenshot validation rejects sparse and transparent contentless frames")

if emu:
    debug_scripts = "\n".join(str((step.get("with") or {}).get("script", ""))
                              for step in steps((emu.get("jobs") or {}).get("emulator") or {})
                              if (step.get("uses") or "").startswith("reactivecircus/android-emulator-runner"))
    if "android-emulator-legs.sh" in debug_scripts:
        debug_scripts += "\n" + pathlib.Path("scripts/release/android-emulator-legs.sh").read_text()
    configured_scripts = "\n".join(str((step.get("with") or {}).get("script", ""))
                                   for step in steps((emu.get("jobs") or {}).get("release-emulator") or {})
                                   if (step.get("uses") or "").startswith("reactivecircus/android-emulator-runner"))
    if "android-release-emulator-legs.sh" in configured_scripts:
        configured_scripts += "\n" + pathlib.Path("scripts/release/android-release-emulator-legs.sh").read_text()
    configured_env = next((step.get("env") or {} for step in steps((emu.get("jobs") or {}).get("release-emulator") or {})
                           if (step.get("uses") or "").startswith("reactivecircus/android-emulator-runner")), {})
    rec("android-cloud-evidence.sh" in debug_scripts and "--unconfigured" in debug_scripts,
        "android-emulator.yml captures the unconfigured cloud state",
        "the debug emulator leg must retain the build-without-deployment state")
    rec("android-cloud-evidence.sh" in configured_scripts and configured_env.get("CLOUD_MODE") == "configured",
        "android-emulator.yml captures the configured cloud lifecycle",
        "the minified emulator fixture must exercise sign-in, sync, offline, and sign-out")

cloud_scenarios = {
    "android-cloud-evidence.sh": pathlib.Path("scripts/release/android-cloud-evidence.sh").read_text(),
    "macos-cloud-evidence.sh": pathlib.Path("scripts/release/macos-cloud-evidence.sh").read_text(),
}
for name, body in cloud_scenarios.items():
    required_states = {"unconfigured", "signed-out", "signed-in", "sync-with-skips", "offline-error", "signed-out-again"}
    rec(all("capture_state {}".format(state) in body for state in required_states),
        "{} captures every cloud evidence state".format(name),
        "required states: {}".format(sorted(required_states)))
    required_actions = {"Sign in", "Connected", "Sync cloud now", "skipped", "The last cloud sync failed", "Sign out"}
    rec(all(action in body for action in required_actions),
        "{} drives the cloud account lifecycle".format(name),
        "required actions: {}".format(sorted(required_actions)))
    rec("cloud_latency_write" in body and body.count("cloud_latency_record") >= 4,
        "{} writes measured cloud latency evidence".format(name),
        "status, sign-in, sync, and offline error must each be timed")

NO_DEVICE = "its detectors are the only part checkable without a device, so they have to be checkable"
SELF_TESTED = {
    "android-smoke.sh": NO_DEVICE,
    "android-smoke-release.sh": NO_DEVICE,
    "android-storage-transfer.sh": "its accessibility selectors must be fixture-tested without borrowing the owned emulator",
    "android-cloud-evidence.sh": "its accessibility selectors and latency verdicts must be fixture-tested without a device",
    "macos-cloud-evidence.sh": "its accessibility evidence and latency verdicts must be fixture-tested off macOS",
    "macos-native-evidence.sh": "its accessibility detector must be fixture-tested off macOS",
    "android-rungs.sh": NO_DEVICE,
    "png_evidence.py": "its content threshold and alpha handling must be fixture-tested",
    "check-wiring.py": "the runner-image table is data, and nothing else would notice it going empty",
}
for name, why in SELF_TESTED.items():
    script = pathlib.Path("scripts/release") / name
    rec(script.is_file() and "--self-test" in script.read_text(),
        "{} carries a --self-test".format(name), why)
    rec("{} --self-test".format(name) in pathlib.Path("scripts/release/check.sh").read_text(),
        "check.sh runs {} --self-test".format(name),
        "otherwise nothing ever proves the detectors report a failure when there is one")
ledger_check = pathlib.Path("scripts/check-feature-ledger.py").read_text()
release_check = pathlib.Path("scripts/release/check.sh").read_text()
rec("--self-test" in ledger_check and "check-feature-ledger.py --self-test" in release_check,
    "the feature-ledger schema carries a wired self-test",
    "the schema detector must prove its negative performance and cloud fixtures")
ci_ledger_body = "\n".join(step.get("run") or "" for step in steps(ci_jobs.get("feature-ledger") or {}))
rec("check-feature-ledger.py --self-test" in ci_ledger_body
    and "check-feature-ledger.py" in ci_ledger_body,
    "ci.yml runs the feature-ledger provenance fixtures and guard",
    "CI must exercise the detector before trusting the ledger")
release_ledger_body = "\n".join(step.get("run") or "" for step in steps(release_jobs.get("native-parity") or {}))
rec("check-feature-ledger.py" in release_ledger_body,
    "release.yml validates performance provenance before native parity",
    "publication must reject a credited p95 whose producer or wiring went stale")

# --- self-test: prove the runner-image detector fails when it should --------
if SELF_TEST:
    def probe(*runs_on):
        jobs = {"j{}".format(i): {"runs-on": r} for i, r in enumerate(runs_on)}
        return all(cond for cond, _, _ in runner_image_checks({"probe.yml": {"jobs": jobs}}))

    def probe_matrix(*runners):
        jobs = {"m": {"runs-on": "${{ matrix.runner }}",
                      "strategy": {"matrix": {"runner": list(runners)}}}}
        return all(cond for cond, _, _ in runner_image_checks({"probe.yml": {"jobs": jobs}}))

    for desc, held in (
        ("the shipping Windows image is recognised", probe("windows-2022")),
        ("a mistyped label is not", not probe("windows-2202")),
        ("a retired image is not", not probe("windows-2019")),
        ("two macOS images in one tree are mixed", not probe("macos-14", "macos-15")),
        ("two Windows images in one tree are mixed", not probe("windows-2022", "windows-2025")),
        ("two Linux images in one tree are not", probe("ubuntu-latest", "ubuntu-24.04")),
        ("a self-hosted label array is left to its own gate",
         probe(["self-hosted", "linux", "ARM64", "android-device"])),
        ("a matrix may span two images of one family", probe_matrix("macos-14", "macos-15")),
        ("a mistyped label in a matrix is not", not probe_matrix("macos-14", "macos-51")),
        ("an empty matrix is not", not probe_matrix()),
    ):
        emit(held, "self-test: {}".format(desc), "the detector did not behave as stated")

    fixture = yaml.safe_load(
        "jobs:\n  build: {}\n  smoke:\n    needs: build\n  publish:\n    needs: [smoke]\n"
    )["jobs"]

    def rejects_bad_yaml():
        try:
            yaml.safe_load("jobs: [")
        except yaml.YAMLError:
            return True
        return False

    for desc, held in (
        ("needs is followed transitively", closure(fixture, "publish") == {"build", "smoke"}),
        ("a scalar needs is one dependency", as_list("build") == ["build"]),
        ("an absent needs is none", as_list(None) == []),
        ("an engines range takes its lowest alternative", min_major(">=20 || ^24") == 20),
        ("unparseable workflow YAML is rejected", rejects_bad_yaml()),
    ):
        emit(held, "self-test: {}".format(desc), "the helper did not behave as stated")
sys.exit(0)
