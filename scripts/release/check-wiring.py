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

WF = pathlib.Path(".github/workflows")
docs = {p.name: yaml.safe_load(p.read_text()) for p in sorted(WF.glob("*.yml"))}
text = {p.name: p.read_text() for p in sorted(WF.glob("*.yml"))}


def rec(cond, desc, detail=""):
    print("{}|{}|{}".format("PASS" if cond else "FAIL", desc, "" if cond else detail))


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


release_jobs = docs["release.yml"].get("jobs") or {}
gate = release_jobs.get("supabase-gate") or {}
gate_body = "\n".join(step.get("run") or "" for step in steps(gate))
rec("real-supabase.sh" in gate_body,
    "release.yml runs the disposable real-Supabase gate")
rec("supabase-gate" in closure(release_jobs, "publish"),
    "release.yml blocks publish on the real-Supabase gate")


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
KNOWN = {"ubuntu-latest", "ubuntu-24.04", "ubuntu-22.04", "macos-14", "macos-15", "macos-latest"}
mac = set()
for wf, doc in docs.items():
    for jn, j in (doc.get("jobs") or {}).items():
        r = j.get("runs-on")
        if not isinstance(r, str):
            continue
        rec(r in KNOWN, "{}: {} runs on a known image ({})".format(wf, jn, r), "unrecognised runner label")
        if r.startswith("macos"):
            mac.add(r)
rec(len(mac) <= 1, "every macOS job uses the same runner image {}".format(sorted(mac)),
    "mixed macOS runners: {}".format(sorted(mac)))

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
def min_major(rng):
    # Lowest major that can satisfy the range: the smallest lower bound across
    # its `||` alternatives. Deliberately crude — it only has to be right for
    # the ">= N" / "^N" shapes npm lockfiles actually contain.
    lows = []
    for alt in rng.split("||"):
        m = re.search(r"(?:>=?|\^|~)?\s*(\d+)", alt)
        if m:
            lows.append(int(m.group(1)))
    return min(lows) if lows else 0


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
            rec(body.lstrip().startswith("set -euo pipefail"),
                "{}: {} step {} opens with set -euo pipefail".format(wf, jn, i),
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
    rec("schedule" in triggers and "workflow_dispatch" in triggers,
        "android-emulator.yml runs nightly and on demand", repr(sorted(triggers)))

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
            rec(script in str(with_.get("script", "")),
                "{} runs scripts/release/{}".format(emulator_job, script),
                "assertions belong in a script check.sh can parse and self-test, not in YAML: {!r}".format(with_.get("script")))
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
    rec("arm64-v8a" in bodies and "android-smoke-release.sh" in bodies,
        "the hardware gate verifies arm64 and runs the release smoke harness",
        "the gate must reject another ABI and exercise the signed APK")

for name in ("android-smoke.sh", "android-smoke-release.sh"):
    smoke = pathlib.Path("scripts/release") / name
    rec(smoke.is_file() and "--self-test" in smoke.read_text(),
        "{} carries a --self-test".format(name),
        "its detectors are the only part checkable without a device, so they have to be checkable")
    rec("{} --self-test".format(name) in pathlib.Path("scripts/release/check.sh").read_text(),
        "check.sh runs {} --self-test".format(name),
        "otherwise nothing ever proves the detectors report a failure when there is one")
sys.exit(0)
