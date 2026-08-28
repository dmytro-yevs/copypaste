#!/usr/bin/env python3
# check-wiring.py — structural checks over .github/workflows/, for check.sh.
#
# Prints one `PASS|description|` or `FAIL|description|detail` line per check and
# always exits 0; check.sh counts them. Run from the repository root.
#
# Everything here is a mistake that only a real run would otherwise report, one
# round trip at a time: an artifact name that does not match its producer, an
# output nothing declares, a job that reads a file no job it depends on wrote.
import copy, json, pathlib, re, shlex, subprocess, sys, yaml

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "ci"))

from gate_registry import load_registry, workspace_contract

from ci_contract import (
    ci_rust_toolchain_holds,
    portable_gate_contract_holds,
    windows_workspace_shards_hold,
)

import native_evidence_wiring

SELF_TEST = "--self-test" in sys.argv
STRICT = "--strict" in sys.argv

WF = pathlib.Path(".github/workflows")


SELF_TEST_FAILURES = 0


def emit(cond, desc, detail=""):
    global SELF_TEST_FAILURES
    if not cond:
        SELF_TEST_FAILURES += 1
    print("{}|{}|{}".format("PASS" if cond else "FAIL", desc, "" if cond else detail))


def rec(cond, desc, detail=""):
    # --self-test reports only its own verdicts, so check.sh counts two runs
    # rather than every check in this file twice.
    if not SELF_TEST:
        emit(cond, desc, detail)


def steps(job):
    return job.get("steps") or []


def android_runner_local_artifact_transfers(jobs):
    tokens = (
        "android-release-scaffold",
        "tauri.settings.gradle",
        "tauri.build.gradle.kts",
        "tauri.properties",
        "proguard-tauri.pro",
        "local.properties",
    )
    violations = []
    for job_name, job in jobs.items():
        for step in steps(job):
            action = str(step.get("uses") or "")
            if not action.startswith(("actions/upload-artifact", "actions/download-artifact")):
                continue
            settings = step.get("with") or {}
            path = str(settings.get("path", ""))
            detail = "{}\n{}".format(settings.get("name", ""), path)
            for token in tokens:
                if token in detail:
                    violations.append((job_name, token))
            if "src-tauri/gen/android" in path and "jniLibs/" not in path:
                violations.append((job_name, "generated Android tree"))
    return violations


def as_list(v):
    return [v] if isinstance(v, str) else (v or [])


def shell_words(source):
    lexer = shlex.shlex(source, posix=True, punctuation_chars=";&|(){}")
    lexer.commenters = "#"
    lexer.whitespace_split = True
    words = []
    while (word := lexer.get_token()) != lexer.eof:
        words.append((word, lexer.lineno))
    return words


def shell_function_body(source, name):
    match = re.search(
        r"^{}\(\)\s*\{{[^\n]*\n(.*?)^\}}".format(re.escape(name)),
        source,
        re.M | re.S,
    )
    return match.group(1) if match else ""


def adb_guard_violations(source, allowed_raw_adb=0):
    words = shell_words(source)
    raw = [(line, word) for word, line in words if word == "adb"]
    violations = []
    if len(raw) != allowed_raw_adb:
        violations.append("found {} raw adb tokens, expected {}".format(len(raw), allowed_raw_adb))
    controls = {"", "(", ")", "{", "}", ";", "&&", "||", "|"}
    for index, (word, line) in enumerate(words):
        next_word = words[index + 1][0] if index + 1 < len(words) else ""
        if word == "bounded_adb" and "(" not in next_word:
            if next_word != "-s":
                violations.append("line {} calls bounded_adb without -s".format(line))
        if word == "targeted_adb" and "(" not in next_word:
            if next_word in controls or next_word.startswith("-"):
                violations.append("line {} calls targeted_adb without a serial".format(line))
    return violations


def calls_emulator(job, event):
    return (
        str(job.get("uses") or "").endswith("android-emulator.yml")
        and "github.event_name == '{}'".format(event) in str(job.get("if") or "")
    )


def matching_event_path_filters(workflow):
    triggers = workflow.get(True) or workflow.get("on") or {}
    push = triggers.get("push")
    pull_request = triggers.get("pull_request")
    if not isinstance(push, dict) or not isinstance(pull_request, dict):
        return False
    push_paths = push.get("paths")
    pull_request_paths = pull_request.get("paths")
    return (
        isinstance(push_paths, list)
        and isinstance(pull_request_paths, list)
        and push_paths == pull_request_paths
    )


def android_webview_accessibility_contract(build_task, extension, frontend, wrapper):
    compact = re.sub(r"\s+", " ", build_task)
    source = re.search(
        r'val\s+(\w+)\s*=\s*File\(\s*project\.projectDir,\s*'
        r'"src/main/rust-webview-accessibility\.kt\.inc",\s*\)\.readText\(\)',
        compact,
    )
    if not source:
        return False, "BuildTask.kt does not read the tracked RustWebView extension"
    if not re.search(
        r'environment\(\s*"WRY_RUSTWEBVIEW_CLASS_EXTENSION",\s*{}\s*\)'.format(
            re.escape(source.group(1))
        ),
        compact,
    ):
        return False, "BuildTask.kt does not export the file it read to Wry"
    if not re.search(r"override\s+fun\s+getAccessibilityNodeProvider\s*\(\s*\)", extension):
        return False, "the Wry extension does not override getAccessibilityNodeProvider"
    if not re.search(
        r"\.wrap\(\s*super\.getAccessibilityNodeProvider\(\s*\)\s*\)", extension
    ):
        return False, "the Wry extension does not wrap the direct WebView provider"
    frontend_marker = re.search(
        r'const\s+ANDROID_PAIRING_BODY_ID\s*=\s*"([^"]+)"', frontend
    )
    native_marker = re.search(
        r'const\s+val\s+PAIRING_BODY_ACCESSIBILITY_MARKER\s*=\s*"([^"]+)"',
        wrapper,
    )
    if not frontend_marker or not native_marker:
        return False, "the pairing marker is missing from the frontend or native matcher"
    if "info.viewIdResourceName != PAIRING_BODY_ACCESSIBILITY_MARKER" not in wrapper:
        return False, "the native matcher does not require the marker constant"
    if frontend_marker.group(1) != native_marker.group(1):
        return False, "frontend marker {!r} differs from native marker {!r}".format(
            frontend_marker.group(1), native_marker.group(1)
        )
    return True, ""


def retired_push_branch_violations(workflows, retired):
    violations = []
    for name, workflow in workflows.items():
        triggers = workflow.get(True) or workflow.get("on") or {}
        push = triggers.get("push")
        if not isinstance(push, dict):
            continue
        if retired in as_list(push.get("branches")):
            violations.append(name)
    return violations


def job_matrix(job):
    return ((job or {}).get("strategy") or {}).get("matrix") or {}


def shell_array(source, name):
    match = re.search(r"^{}=\(([^)]*)\)".format(re.escape(name)), source, re.M)
    return shlex.split(match.group(1)) if match else []


def android_link_abi_matrix_holds(job, link_source, ndk_source):
    matrix = job_matrix(job)
    triples = matrix.get("triple")
    if not isinstance(triples, list):
        return False
    defaults = shell_array(link_source, "TRIPLES")
    mappings = shell_array(ndk_source, "TRIPLES")
    required, actual = set(defaults), set(triples)
    commands = [
        line.strip()
        for step in steps(job)
        for line in (step.get("run") or "").splitlines()
        if "android-link-abis.sh" in line and not line.lstrip().startswith("#")
    ]
    targets = [
        str((step.get("with") or {}).get("targets", ""))
        for step in steps(job)
        if str(step.get("uses") or "").startswith("dtolnay/rust-toolchain")
    ]
    cache_keys = [
        str((step.get("with") or {}).get("shared-key", ""))
        for step in steps(job)
        if str(step.get("uses") or "").startswith("Swatinem/rust-cache")
    ]
    return (
        set(matrix) == {"triple"}
        and defaults
        and defaults == mappings
        and len(defaults) == len(required)
        and not (required - actual)
        and not (actual - required)
        and len(triples) == len(actual)
        and targets == ["${{ matrix.triple }}"]
        and cache_keys == ["android-link-${{ matrix.triple }}"]
        and commands == ['./scripts/release/android-link-abis.sh "${{ matrix.triple }}"']
    )


def scheduled_sweep_is_single(jobs):
    job = jobs.get("android-nightly") or {}
    return calls_emulator(job, "schedule") and not job_matrix(job)


def dispatch_spot_check_holds(jobs):
    job = jobs.get("android-dispatch") or {}
    return (
        calls_emulator(job, "workflow_dispatch")
        and set(job_matrix(job).get("api-level") or []) == {34, 36}
    )


def nightly_concurrency_holds(workflow):
    concurrency = workflow.get("concurrency")
    if not isinstance(concurrency, dict):
        return False
    group = concurrency.get("group")
    if not isinstance(group, str) or "github.event_name" not in group:
        return False
    groups = {
        event: re.sub(
            r"\$\{\{\s*github\.event_name\s*\}\}", event, group
        )
        for event in ("schedule", "workflow_dispatch")
    }
    return (
        groups["schedule"] != groups["workflow_dispatch"]
        and concurrency.get("cancel-in-progress") is False
    )


PR_PUSH_OR_RUN = (
    "${{ github.event.pull_request.number || "
    "(github.event_name == 'push' && github.ref_name) || github.run_id }}"
)
ACTIVE_CHANGE_CANCEL = (
    "${{ github.event_name == 'pull_request' || github.event_name == 'push' }}"
)
GATE_CONCURRENCY = {
    "ci.yml": "ci",
    "browser-webkitgtk.yml": "browser",
    "windows-native-e2e.yml": "windows-native-e2e",
    "mutation-gate.yml": "mutation-gate",
    "supply-chain.yml": "supply-chain",
}


def event_triggers(workflow):
    return workflow.get(True) or workflow.get("on") or {}


def checks_merge_queue(workflow):
    merge_group = event_triggers(workflow).get("merge_group")
    return isinstance(merge_group, dict) and merge_group.get("types") == ["checks_requested"]


def concurrency_policy_failures(workflows):
    failures = []
    for name, slug in GATE_CONCURRENCY.items():
        workflow = workflows.get(name) or {}
        concurrency = workflow.get("concurrency") or {}
        expected = "{}-${{{{ github.event_name }}}}-{}".format(slug, PR_PUSH_OR_RUN)
        if concurrency.get("group") != expected:
            failures.append("{} must use {}".format(name, expected))
        if concurrency.get("cancel-in-progress") != ACTIVE_CHANGE_CANCEL:
            failures.append("{} may cancel only pull_request and push".format(name))
        if not checks_merge_queue(workflow):
            failures.append("{} must run for merge_group checks_requested".format(name))

    android = workflows.get("android-emulator.yml") or {}
    android_concurrency = android.get("concurrency") or {}
    android_group = (
        "android-emulator-${{ github.event_name }}-" + PR_PUSH_OR_RUN + "-"
        "${{ github.event_name == 'schedule' && 'sweep' || inputs.api-level || '36' }}-"
        "${{ inputs.target || 'google_apis' }}"
    )
    if android_concurrency.get("group") != android_group:
        failures.append("android-emulator.yml must isolate event, change, API sweep, and target")
    if android_concurrency.get("cancel-in-progress") != ACTIVE_CHANGE_CANCEL:
        failures.append("android-emulator.yml may cancel only pull_request and push")
    if not checks_merge_queue(android):
        failures.append("android-emulator.yml must run for merge_group checks_requested")

    release = (workflows.get("release.yml") or {}).get("concurrency") or {}
    release_group = (
        "release-${{ github.event_name == 'workflow_dispatch' && "
        "format('v{0}', inputs.version) || github.ref_name }}"
    )
    if release.get("group") != release_group:
        failures.append("release.yml must serialize the normalized tag or dispatch version")
    if release.get("cancel-in-progress") is not False:
        failures.append("release.yml must not cancel release evidence")

    nightly = workflows.get("native-nightly.yml") or {}
    if not nightly_concurrency_holds(nightly):
        failures.append("native-nightly.yml must isolate schedule and dispatch without cancellation")
    if (workflows.get("secret-scan.yml") or {}).get("concurrency") is not None:
        failures.append("secret-scan.yml must inherit the caller's concurrency")
    return failures


def synthetic_bucket(slug, event, run_id, pull_request=None, ref_name=None,
                     api_level=None, target=None):
    change = pull_request if event == "pull_request" else (
        ref_name if event == "push" else run_id
    )
    fields = [slug, event, str(change)]
    if slug == "android-emulator":
        fields += ["sweep" if event == "schedule" else (api_level or "36"),
                   target or "google_apis"]
    return "-".join(fields)


def synthetic_cancels(event):
    return event in {"pull_request", "push"}


def synthetic_release_bucket(event, ref_name=None, version=None):
    normalized = "v{}".format(version) if event == "workflow_dispatch" else ref_name
    return "release-{}".format(normalized)


def synthetic_concurrency_table_holds():
    pr_one = synthetic_bucket("ci", "pull_request", 10, pull_request=41)
    pr_update = synthetic_bucket("ci", "pull_request", 11, pull_request=41)
    pr_other = synthetic_bucket("ci", "pull_request", 12, pull_request=42)
    push_one = synthetic_bucket("ci", "push", 20, ref_name="main")
    push_update = synthetic_bucket("ci", "push", 21, ref_name="main")
    merge_one = synthetic_bucket("ci", "merge_group", 30, ref_name="gh-readonly-queue/main/a")
    merge_two = synthetic_bucket("ci", "merge_group", 31, ref_name="gh-readonly-queue/main/b")
    dispatch = synthetic_bucket("supply-chain", "workflow_dispatch", 40, ref_name="main")
    schedule = synthetic_bucket("supply-chain", "schedule", 41, ref_name="main")
    android_api = synthetic_bucket(
        "android-emulator", "workflow_dispatch", 50, api_level="34", target="google_apis")
    android_other_api = synthetic_bucket(
        "android-emulator", "workflow_dispatch", 50, api_level="36", target="google_apis")
    android_other_target = synthetic_bucket(
        "android-emulator", "workflow_dispatch", 50, api_level="34", target="aosp_atd")
    android_sweep = synthetic_bucket(
        "android-emulator", "schedule", 60, api_level="34", target="google_apis")
    return all((
        pr_one == pr_update,
        pr_one != pr_other,
        pr_one != synthetic_bucket("browser", "pull_request", 10, pull_request=41),
        push_one == push_update,
        merge_one != merge_two,
        dispatch != schedule,
        android_api != android_other_api,
        android_api != android_other_target,
        android_sweep.endswith("-sweep-google_apis"),
        synthetic_release_bucket("push", ref_name="v2.0.0")
        == synthetic_release_bucket("workflow_dispatch", version="2.0.0"),
        synthetic_cancels("pull_request"),
        synthetic_cancels("push"),
        not synthetic_cancels("merge_group"),
        not synthetic_cancels("workflow_dispatch"),
        not synthetic_cancels("schedule"),
    ))


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

nightly_workflow = docs.get("native-nightly.yml") or {}
rec(nightly_concurrency_holds(nightly_workflow),
    "native-nightly.yml isolates scheduled and manual evidence",
    "group must include github.event_name and cancel-in-progress must be false")
concurrency_failures = concurrency_policy_failures(docs)
rec(not concurrency_failures,
    "workflow concurrency isolates events and preserves merge-queue evidence",
    "; ".join(concurrency_failures))
rec(synthetic_concurrency_table_holds(),
    "workflow concurrency synthetic event table preserves only intended supersession")

try:
    WORKSPACE_PACKAGES, RUST_VERSION = workspace_contract(pathlib.Path.cwd())
    WORKSPACE_METADATA_ERROR = ""
except (KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
    WORKSPACE_PACKAGES, RUST_VERSION = set(), ""
    WORKSPACE_METADATA_ERROR = str(error)

try:
    CI_GATES = load_registry(pathlib.Path.cwd())
    CI_GATES_ERROR = ""
except (OSError, ValueError) as error:
    CI_GATES, CI_GATES_ERROR = {}, str(error)


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
rec(not WORKSPACE_METADATA_ERROR,
    "Cargo metadata resolves the workspace package and Rust contracts",
    WORKSPACE_METADATA_ERROR)
rec(windows_workspace_shards_hold(ci_jobs, WORKSPACE_PACKAGES, RUST_VERSION),
    "ci.yml shards every Windows package under 15 minutes with isolated cache writers",
    "test shards must cover each workspace package once with default features and unique cache keys")
toolchains_hold, toolchain_failures = ci_rust_toolchain_holds(docs["ci.yml"], RUST_VERSION)
rec(toolchains_hold,
    "ci.yml Rust pins equal Cargo rust-version",
    "mismatched pins: {}".format(toolchain_failures))
rec(not CI_GATES_ERROR and portable_gate_contract_holds(
        CI_GATES,
        "linux-ci-mirror",
        pathlib.Path("scripts/prepush/wsl/verify.sh").read_text(),
        ci_jobs,
    ),
    "CI and the WSL mirror select the same enforcing portable gate registry",
    CI_GATES_ERROR or "a gate is missing, duplicated, or replaced by an advisory command")
documentation = ci_jobs.get("documentation") or {}
documentation_body = "\n".join(step.get("run") or "" for step in steps(documentation))
rec("check-docs.py" in documentation_body,
    "ci.yml gates documentation links and unfinished-work markers")
rec("check-docs.test.py" in documentation_body,
    "ci.yml runs the protocol-version guard self-test")
retired_branch_workflows = retired_push_branch_violations(docs, "v2-main")
rec(not retired_branch_workflows,
    "workflows do not target the retired v2-main branch",
    "push filters still name v2-main: {}".format(retired_branch_workflows))


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

# --- job execution budgets --------------------------------------------------
def job_timeout_checks(workflows, maximum=20):
    for wf, doc in workflows.items():
        for jn, job in (doc.get("jobs") or {}).items():
            if "uses" in job:
                # A reusable caller has no runner of its own. Its callee owns
                # the timeout, so classify it separately from an unbounded job.
                yield (True,
                       "{}: {} delegates its timeout to a reusable workflow".format(wf, jn),
                       "the called workflow owns the runner budget")
                continue
            if "runs-on" not in job:
                continue
            timeout = job.get("timeout-minutes")
            yield (isinstance(timeout, int) and not isinstance(timeout, bool)
                   and 0 < timeout <= maximum,
                   "{}: {} is bounded to at most {} minutes".format(wf, jn, maximum),
                   "timeout-minutes is {!r}".format(timeout))


for check in job_timeout_checks(docs):
    rec(*check)


def cargo_deny_contract(workflow):
    job = (workflow.get("jobs") or {}).get("deny") or {}
    strategy = job.get("strategy") or {}
    matrix = (strategy.get("matrix") or {}).get("check")
    job_steps = steps(job)
    docker_actions = [
        step for step in job_steps
        if str(step.get("uses") or "").split("@", 1)[0].lower()
        == "embarkstudios/cargo-deny-action"
    ]
    installers = [
        step for step in job_steps
        if str(step.get("uses") or "").split("@", 1)[0]
        == "taiki-e/install-action"
        and str(step.get("with", {}).get("tool") or "").startswith("cargo-deny")
    ]
    settings = (installers[0].get("with") or {}) if len(installers) == 1 else {}
    install_ref = str(installers[0].get("uses") or "") if len(installers) == 1 else ""
    commands = [
        str(step.get("run") or "") for step in job_steps
        if "cargo deny" in str(step.get("run") or "")
    ]
    command = " ".join(commands[0].split()) if len(commands) == 1 else ""
    required_prefix = (
        "cargo deny --manifest-path Cargo.toml --config deny.toml "
        "--all-features --locked check "
    )
    required_suffix = " --show-stats"
    flags_hold = command.startswith(required_prefix) and command.endswith(required_suffix)
    selector_match = re.search(r"\bcheck\s+(.+?)\s+--show-stats$", command)
    selector = selector_match.group(1) if selector_match else ""
    return {
        "matrix": matrix == ["advisories", "bans", "licenses", "sources"],
        "fail-fast": strategy.get("fail-fast") is False,
        "dockerless": not docker_actions,
        "installer-count": len(installers) == 1,
        "installer-ref": install_ref == (
            "taiki-e/install-action@6a1bd70eaac3c8bdf093356838d7ee09fda951cf"
        ),
        "version": settings.get("tool") == "cargo-deny@0.20.2",
        "checksum": str(settings.get("checksum", "")).lower() == "true",
        "fallback": settings.get("fallback") == "none",
        "command-count": len(commands) == 1,
        "flags": flags_hold,
        "selector": selector == "${{ matrix.check }}",
    }


deny_checks = cargo_deny_contract(docs.get("supply-chain.yml") or {})
for key, description in (
    ("matrix", "runs the four policy checks as separate matrix legs"),
    ("fail-fast", "keeps every policy leg visible after a failure"),
    ("dockerless", "does not use the Docker-based cargo-deny action"),
    ("installer-count", "installs cargo-deny exactly once"),
    ("installer-ref", "pins the cargo-deny installer to the reviewed full SHA"),
    ("version", "pins cargo-deny 0.20.2"),
    ("checksum", "verifies the cargo-deny release checksum"),
    ("fallback", "fails instead of compiling an unverified fallback"),
    ("command-count", "runs cargo deny exactly once per matrix leg"),
    ("flags", "uses the manifest, policy, feature, lockfile, and stats flags"),
    ("selector", "runs the selected matrix check instead of a hardcoded subset"),
):
    rec(deny_checks[key], "supply-chain.yml: cargo-deny {}".format(description),
        "the Dockerless cargo-deny contract is incomplete")

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
# floor to 24 while every job pinned 22. The pin lives in .nvmrc; every
# setup-node step must read it via node-version-file so majors cannot drift
# job-by-job again.
NVMRC = pathlib.Path(".nvmrc")
nvmrc_text = NVMRC.read_text().strip() if NVMRC.is_file() else ""
nvmrc_major_m = re.match(r"(\d+)", nvmrc_text)
rec(bool(nvmrc_major_m), ".nvmrc pins a Node major ({})".format(nvmrc_text or "<missing>"),
    "create .nvmrc with a major like 24; workflows read it via node-version-file")
canonical_major = int(nvmrc_major_m.group(1)) if nvmrc_major_m else 0

locks = {}
for wf, doc in docs.items():
    for jn, j in (doc.get("jobs") or {}).items():
        for s in steps(j):
            if not (s.get("uses") or "").startswith("actions/setup-node"):
                continue
            with_ = s.get("with") or {}
            file_pin = str(with_.get("node-version-file", "")).strip()
            hardcoded = str(with_.get("node-version", "")).strip()
            rec(not hardcoded,
                "{}: {} does not hardcode node-version".format(wf, jn),
                "use node-version-file: '.nvmrc' instead of node-version: {!r}".format(hardcoded))
            rec(file_pin in (".nvmrc", "./.nvmrc"),
                "{}: {} reads Node from .nvmrc ({})".format(wf, jn, file_pin or "<unset>"),
                "setup-node must set node-version-file: '.nvmrc'")
            if not canonical_major:
                continue
            major = canonical_major
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

# --- the toolchain components a workspace test run needs --------------------
# `cargo test --workspace` builds tools/sensitive-rules, whose regeneration
# tests spawn `rustfmt` as a child process. rust-toolchain.toml requests the
# component, but dtolnay/rust-toolchain installs only what the step names, so a
# job that omits it passes every other gate and fails there alone: nightly run
# 31671766432 died that way on macos-14 and macos-15 at once.
TEST_SPAWNED_COMPONENTS = {"rustfmt": pathlib.Path("tools/sensitive-rules/src/main.rs")}

for component, source in sorted(TEST_SPAWNED_COMPONENTS.items()):
    rec(source.is_file() and 'Command::new("{}")'.format(component) in source.read_text(),
        "{} still spawns {}".format(source, component),
        "the requirement below is derived from this call; retire the pair together")


def toolchain_component_checks(workflows, required):
    for wf, doc in workflows.items():
        for jn, j in (doc.get("jobs") or {}).items():
            setups = [s for s in steps(j) if (s.get("uses") or "").startswith("dtolnay/rust-toolchain")]
            if not setups:
                continue
            if not any(re.search(r"\bcargo\s+(?:\+\S+\s+)?test\b.*--workspace", line)
                       for s in steps(j) for line in (s.get("run") or "").splitlines()):
                continue
            installed = set()
            for s in setups:
                installed |= {c.strip() for c in
                              str((s.get("with") or {}).get("components", "")).split(",") if c.strip()}
            for component in sorted(required):
                yield (component in installed,
                       "{}: {} installs {} for cargo test --workspace".format(wf, jn, component),
                       "components installed: {}".format(sorted(installed)))


for check in toolchain_component_checks(docs, TEST_SPAWNED_COMPONENTS):
    rec(*check)

# --- the generated Android WebView provider seam ---------------------------
ANDROID_WEBVIEW_BUILD_TASK = pathlib.Path(
    "crates/copypaste-ui/src-tauri/gen/android/buildSrc/src/main/java/"
    "com/copypaste/app/kotlin/BuildTask.kt"
)
ANDROID_WEBVIEW_EXTENSION = pathlib.Path(
    "crates/copypaste-ui/src-tauri/gen/android/app/src/main/"
    "rust-webview-accessibility.kt.inc"
)
ANDROID_PAIRING_FRONTEND = pathlib.Path(
    "crates/copypaste-ui/src/features/devices/patterns/PairingLauncherDialog.tsx"
)
ANDROID_PAIRING_WRAPPER = pathlib.Path(
    "crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/"
    "com/copypaste/app/PairingBackdropAccessibility.kt"
)


def read_contract_source(source):
    return source.read_text() if source.is_file() else ""


ANDROID_WEBVIEW_SOURCES = tuple(
    read_contract_source(source)
    for source in (
        ANDROID_WEBVIEW_BUILD_TASK,
        ANDROID_WEBVIEW_EXTENSION,
        ANDROID_PAIRING_FRONTEND,
        ANDROID_PAIRING_WRAPPER,
    )
)
android_webview_held, android_webview_detail = \
    android_webview_accessibility_contract(*ANDROID_WEBVIEW_SOURCES)
rec(android_webview_held,
    "Android pairing accessibility wraps Wry's direct provider seam",
    android_webview_detail)

# --- the Android NDK binutils wiring ---------------------------------------
# openssl-src asks cc-rs for AR and RANLIB, and cc-rs falls back to
# `<triple>-ranlib` — a wrapper no NDK has shipped since r23 — unless something
# exports RANLIB_<triple>. android-ndk-env.sh exports it, per triple, so its
# list and the list of targets the job installs have to stay the same set: a
# target it misses fails only after several minutes of OpenSSL build.
android_jobs = docs["release.yml"].get("jobs") or {}
android = android_jobs.get("android-abi") or {}
wiring = [s for s in steps(android) if "android-ndk-env.sh" in (s.get("run") or "")]
rec(len(wiring) == 1, "release.yml: android-abi runs android-ndk-env.sh exactly once",
    "found {} steps invoking it".format(len(wiring)))
rec(any("GITHUB_ENV" in (s.get("run") or "") for s in wiring),
    "release.yml: android-abi android-ndk-env.sh output reaches GITHUB_ENV",
    "the script only prints; nothing reads it unless it is appended to GITHUB_ENV")
installed = {
    entry.get("triple")
    for entry in (((android.get("strategy") or {}).get("matrix") or {}).get("include") or [])
}
for setup in steps(android_jobs.get("android") or {}):
    if (setup.get("uses") or "").startswith("dtolnay/rust-toolchain"):
        installed |= {
            target.strip()
            for target in str((setup.get("with") or {}).get("targets", "")).split(",")
            if target.strip()
        }
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
    rec(matching_event_path_filters(emu),
        "android-emulator.yml push and pull-request path filters match",
        "the duplicated lists must stay structurally identical")
    for event in ("push", "pull_request"):
        paths = (triggers.get(event) or {}).get("paths") or []
        rec(any(p.startswith("crates/copypaste-ui/src/") for p in paths)
            and any(p.startswith("crates/copypaste-ipc") for p in paths),
            f"android-emulator.yml {event} filter covers the shared frontend and the wire contract",
            "a path filter that omits them hides cross-platform breakage")
    nightly_jobs = nightly_workflow.get("jobs") or {}
    rec("workflow_call" in triggers and "workflow_dispatch" in triggers,
        "android-emulator.yml is reusable and dispatchable",
        "expected workflow_call and workflow_dispatch triggers: {}".format(sorted(triggers)))
    rec(dispatch_spot_check_holds(nightly_jobs),
        "a manual nightly run spot-checks the API 34/36 matrix",
        "expected a workflow_dispatch-gated call to android-emulator.yml matrixed on api-level [34, 36]")
    # `github.event_name` inside a called workflow is the CALLER's event, so on
    # a schedule the emulator job's own matrix expands to [24,29,33,34,36] and
    # ignores any api-level passed in. A matrix on the scheduled call therefore
    # repeats the whole sweep once per leg — run 31774201631 ran ten emulator
    # legs and two APK builds for what is one sweep's worth of coverage.
    rec(scheduled_sweep_is_single(nightly_jobs),
        "the scheduled Android call is not matrixed",
        "the callee already sweeps every API level on a schedule; a matrix here runs that sweep once per leg")

    emulator_matrix = ((ejobs.get("emulator") or {}).get("strategy") or {}).get("matrix") or {}
    api_matrix = str(emulator_matrix.get("api-level", ""))
    rec("[24,29,33,34,36]" in api_matrix,
        "android-emulator.yml schedules the representative API matrix",
        "expected 24, 29, 33, 34 and 36 in the scheduled matrix: {!r}".format(api_matrix))
    abi_link = ejobs.get("link-abis") or {}
    abi_link_defaults = pathlib.Path("scripts/release/android-link-abis.sh").read_text()
    ndk_mappings = pathlib.Path("scripts/release/android-ndk-env.sh").read_text()
    rec(android_link_abi_matrix_holds(abi_link, abi_link_defaults, ndk_mappings),
        "android-emulator.yml ABI link matrix matches NDK targets",
        "expected one isolated job for every shared android-link-abis/NDK target: {!r}".format(
            job_matrix(abi_link).get("triple")
        ))

    # Both legs, and the build flag that separates them. The debug leg must
    # stay debuggable or it loses run-as and every filesystem assertion with
    # it; the release leg must stay *not* debuggable or R8 never runs and it
    # becomes a slower copy of the debug one.
    for emulator_job, apk_job, script, debug in (
        ("emulator", "apk", "android-smoke.sh", True),
        ("release-emulator", "release-apk-shard", "android-smoke-release.sh", False),
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
hardware_valid, hardware_detail = native_evidence_wiring.physical_android_contract(release)
rec(hardware_valid,
    "release.yml requires physical arm64 Android evidence",
    hardware_detail)
publish_needs = (release_jobs.get("publish") or {}).get("needs") or []
publish_needs = publish_needs if isinstance(publish_needs, list) else [publish_needs]
native_parity_needs = (release_jobs.get("native-parity") or {}).get("needs") or []
native_parity_needs = native_parity_needs if isinstance(native_parity_needs, list) else [native_parity_needs]
rec("android-hardware" in native_parity_needs and "native-parity" in publish_needs,
    "publishing requires the physical Android hardware gate through native parity",
    repr({"native-parity": native_parity_needs, "publish": publish_needs}))
rec("android-smoke" in publish_needs,
    "publishing retains the Android emulator compatibility gate", repr(publish_needs))

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
release_upgrade = release_jobs.get("android-upgrade-fixture") or {}
release_upgrade_uploads = [str((step.get("with") or {}).get("name", ""))
                           for step in steps(release_upgrade)
                           if (step.get("uses") or "").startswith("actions/upload-artifact")]
release_smoke_downloads = [str((step.get("with") or {}).get("name", ""))
                           for step in steps(release_smoke)
                           if (step.get("uses") or "").startswith("actions/download-artifact")]
release_upgrade_body = "\n".join(step.get("run") or "" for step in steps(release_upgrade))
rec("android-upgrade-fixture" in release_upgrade_uploads
    and "android-upgrade-fixture" in release_smoke_downloads
    and "android-upgrade-fixture" in closure(release_jobs, "android-smoke")
    and release_env.get("PREVIOUS_APK") == "upgrade-dist/copypaste-previous-release.apk"
    and "--write-overlay" in release_upgrade_body
    and "--target x86_64" in release_upgrade_body,
    "release.yml tests a separately signed previous-version APK",
    "the publish workflow must build one emulator ABI, sign, upload, download and install its own upgrade fixture")
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
release_abi = release_jobs.get("android-abi") or {}
release_abi_matrix = ((release_abi.get("strategy") or {}).get("matrix") or {}).get("include") or []
release_abi_targets = {entry.get("target") for entry in release_abi_matrix}
release_android_body = "\n".join(step.get("run") or "" for step in steps(release_android))
release_android_downloads = {
    str((step.get("with") or {}).get("name", ""))
    for step in steps(release_android)
    if (step.get("uses") or "").startswith("actions/download-artifact")
}
local_settings_step = next(
    (index for index, step in enumerate(steps(release_android))
     if step.get("name") == "Generate runner-local Tauri Gradle settings"),
    -1,
)
native_download_steps = [
    index
    for index, step in enumerate(steps(release_android))
    if (step.get("uses") or "").startswith("actions/download-artifact")
]
rec(release_abi_targets == {"aarch64", "armv7", "i686"}
    and "android-abi" in closure(release_jobs, "android")
    and {
        "android-native-aarch64",
        "android-native-armv7",
        "android-native-i686",
    } <= release_android_downloads
    and "npm run tauri -- android build --apk --target x86_64" in release_android_body
    and local_settings_step >= 0
    and native_download_steps
    and all(local_settings_step < index for index in native_download_steps)
    and "assembleUniversalRelease" in release_android_body
    and "-x rustBuildUniversalRelease" in release_android_body
    and "for abi in arm64-v8a armeabi-v7a x86 x86_64" in release_android_body,
    "release.yml shards and reassembles the published universal APK",
    "three remote shards and one runner-local x86_64 build must feed the universal package")
local_android_transfers = android_runner_local_artifact_transfers(release_jobs)
rec(not local_android_transfers,
    "release.yml regenerates machine-local Android settings on the assembly runner",
    "runner-local generated settings crossed an artifact boundary: {!r}".format(local_android_transfers))
rec(bool(re.search(r"^\s*npm run tauri -- android build --apk --target x86_64\s*$",
                   emulator_android_workflow, re.M)),
    "android-emulator.yml rebuilds the shipped APK without cloud evidence configuration")
emulator_shard = ejobs.get("release-apk-shard") or {}
emulator_variants = set(
    (((emulator_shard.get("strategy") or {}).get("matrix") or {}).get("variant") or [])
)
emulator_sign = ejobs.get("release-apk") or {}
emulator_sign_downloads = {
    str((step.get("with") or {}).get("name", ""))
    for step in steps(emulator_sign)
    if (step.get("uses") or "").startswith("actions/download-artifact")
}
rec(emulator_variants == {"shipped", "upgrade", "cloud"}
    and "release-apk-shard" in closure(ejobs, "release-apk")
    and {
        "android-release-unsigned",
        "android-upgrade-unsigned",
        "android-cloud-unsigned",
    } <= emulator_sign_downloads
    and str(((next(
        (step for step in steps(emulator_shard)
         if (step.get("uses") or "").startswith("Swatinem/rust-cache")),
        {},
    ).get("with") or {}).get("shared-key", ""))).endswith("${{ matrix.variant }}"),
    "android-emulator.yml builds release APK variants in isolated shards",
    "shipped, upgrade, and cloud builds need distinct caches and one signing join")

release_windows = release_jobs.get("windows") or {}
windows_body = "\n".join(step.get("run") or "" for step in steps(release_windows))
windows_downloads = [str((step.get("with") or {}).get("name", ""))
                     for step in steps(release_windows)
                     if (step.get("uses") or "").startswith("actions/download-artifact")]
rec("windows-sidecars" in closure(release_jobs, "windows")
    and "windows-release-sidecars" in windows_downloads
    and "-PrebuiltSidecarsDirectory artifacts/windows-sidecars" in windows_body,
    "release.yml builds Windows sidecars in parallel with the app",
    "the installer job must depend on, download and pass the prebuilt sidecar artifact")

ui_cargo = pathlib.Path("crates/copypaste-ui/src-tauri/Cargo.toml").read_text()
embedded_cloud = pathlib.Path("crates/copypaste-ui/src-tauri/src/backend/embedded/cloud.rs").read_text()
android_cloud = pathlib.Path("scripts/release/android-cloud-evidence.sh").read_text()
android_ui_evidence = pathlib.Path("scripts/release/android-ui-evidence-lib.sh").read_text()
android_screencap = pathlib.Path("scripts/release/android-screencap-lib.sh").read_text()
android_release = pathlib.Path("scripts/release/android-smoke-release.sh").read_text()
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
timeout_words = [word for word, _ in shell_words(shell_function_body(
    android_screencap, "run_with_android_screencap_timeout"))]
bounded_words = [word for word, _ in shell_words(shell_function_body(android_screencap, "bounded_adb"))]
targeted_words = [word for word, _ in shell_words(shell_function_body(android_screencap, "targeted_adb"))]
production_screencap = android_screencap.partition("android_screencap_self_test()")[0]
adb_violations = adb_guard_violations(production_screencap, allowed_raw_adb=1)
rec(timeout_words[:2] == ["timeout", "--foreground"] and timeout_words[-1:] == ["$@"]
    and bounded_words == ["MSYS2_ARG_CONV_EXCL=$ANDROID_DEVICE_PATH_EXCL",
                          "run_with_android_screencap_timeout", "adb", "$@"]
    and ["bounded_adb", "-s", "$serial", "$@"] == targeted_words[-4:]
    and not adb_violations,
    "Android screenshot adb calls use the targeted bounded wrapper",
    "; ".join(adb_violations)
    or "timeout/bounded/targeted/path-exclusion wrapper composition changed")
android_adb = pathlib.Path("scripts/release/android-adb-lib.sh").read_text()
android_smoke_lib = pathlib.Path("scripts/release/android-smoke-lib.sh").read_text()
android_transfer = pathlib.Path("scripts/release/android-storage-transfer.sh").read_text()
rec("MSYS2_ARG_CONV_EXCL" in android_adb
    and "android_adb_self_test" in android_ui_evidence
    and "sh_() { adb_ shell" in android_smoke_lib
    and "adb_ shell uiautomator dump" in android_smoke_lib
    and "adb_ pull" in android_smoke_lib
    and "adb shell dd" not in android_transfer,
    "Android device paths survive a host shell that rewrites POSIX arguments",
    "Git Bash rewrites /sdcard into C:/Program Files/Git/sdcard before adb sees it")
rec("verified_android_serial" in android_screencap
    and "ANDROID_SERIAL" in android_screencap and "EMULATOR_PORT" in android_screencap
    and "append_bounded_adb_diagnostic" in android_screencap
    and "route=trusted-serial" in android_screencap
    and '[[ "$serial" == emulator-* ]]' in android_screencap
    and "Screenshot_*.png" in android_screencap
    and 'for attempt in $(seq 1 "$ANDROID_SCREENCAP_ATTEMPTS")' in android_screencap
    and "transport retries exhausted" in android_screencap
    and "failed PNG evidence validation; not retried" in android_screencap,
    "Android emulator evidence uses bounded host transport retries",
    "guest adb screencap can return zero-byte or black frames while the host framebuffer is painted")
rec("cleanup_status" in android_screencap and not adb_violations,
    "physical Android evidence retains bounded adb capture and fail-closed cleanup")
receipt_body = android_release[android_release.rfind("if [[ $FAIL -eq 0 ]]"):]
receipt_violations = adb_guard_violations(receipt_body)
rec("verified_android_serial" in receipt_body
    and "release-receipt-route.log" in receipt_body
    and not receipt_violations,
    "Android receipt classification revalidates the targeted bounded route",
    "; ".join(receipt_violations) or "receipt classification bypasses serial verification")
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
rec("scripts/ci/run-gates.py --gate feature-ledger" in ci_ledger_body,
    "ci.yml runs the feature-ledger provenance fixtures and guard",
    "CI must exercise the detector before trusting the ledger")
release_ledger_body = "\n".join(step.get("run") or "" for step in steps(release_jobs.get("native-parity") or {}))
rec("check-feature-ledger.py" in release_ledger_body,
    "release.yml validates performance provenance before native parity",
    "publication must reject a credited p95 whose producer or wiring went stale")

# --- self-test: prove the runner-image detector fails when it should --------
if SELF_TEST:
    current_supply = docs.get("supply-chain.yml") or {}

    def concurrency_fixture(mutator):
        fixture = copy.deepcopy(docs)
        mutator(fixture)
        return bool(concurrency_policy_failures(fixture))

    def change_group(fixture, name, old, new):
        group = fixture[name]["concurrency"]["group"]
        fixture[name]["concurrency"]["group"] = group.replace(old, new)

    concurrency_fixtures = (
        (
            "a bare ref concurrency key is rejected",
            lambda fixture: fixture["ci.yml"]["concurrency"].update(
                group="ci-${{ github.ref }}"),
        ),
        (
            "a workflow-derived concurrency slug is rejected",
            lambda fixture: change_group(fixture, "ci.yml", "ci-", "${{ github.workflow }}-"),
        ),
        (
            "a missing merge-queue trigger is rejected",
            lambda fixture: event_triggers(fixture["browser-webkitgtk.yml"]).pop(
                "merge_group", None),
        ),
        (
            "merge-queue evidence may not be cancelled",
            lambda fixture: fixture["windows-native-e2e.yml"]["concurrency"].update(
                **{"cancel-in-progress": True}),
        ),
        (
            "nightly schedule and dispatch may not share a group",
            lambda fixture: fixture["native-nightly.yml"]["concurrency"].update(
                group="native-nightly-${{ github.ref }}"),
        ),
        (
            "Android concurrency retains the schedule sweep discriminator",
            lambda fixture: change_group(
                fixture, "android-emulator.yml",
                "github.event_name == 'schedule' && 'sweep' || ", ""),
        ),
        (
            "Android concurrency retains the API discriminator",
            lambda fixture: change_group(
                fixture, "android-emulator.yml",
                "${{ github.event_name == 'schedule' && 'sweep' || inputs.api-level || '36' }}-",
                ""),
        ),
        (
            "Android concurrency retains the target discriminator",
            lambda fixture: change_group(
                fixture, "android-emulator.yml", "-${{ inputs.target || 'google_apis' }}", ""),
        ),
        (
            "release concurrency uses the normalized version",
            lambda fixture: fixture["release.yml"]["concurrency"].update(
                group="release-${{ github.ref }}"),
        ),
        (
            "release evidence may not be cancelled",
            lambda fixture: fixture["release.yml"]["concurrency"].update(
                **{"cancel-in-progress": True}),
        ),
        (
            "the reusable secret scan may not own caller concurrency",
            lambda fixture: fixture["secret-scan.yml"].update(concurrency={
                "group": "secret-scan-${{ github.run_id }}",
                "cancel-in-progress": False,
            }),
        ),
    )
    emit(not concurrency_policy_failures(docs),
         "self-test: current workflow concurrency policy holds",
         "; ".join(concurrency_policy_failures(docs)))
    emit(synthetic_concurrency_table_holds(),
         "self-test: synthetic concurrency event table holds")
    for desc, mutate in concurrency_fixtures:
        emit(concurrency_fixture(mutate), "self-test: {}".format(desc),
             "the workflow concurrency detector accepted a broken fixture")

    def broken_deny(mutator):
        fixture = copy.deepcopy(current_supply)
        mutator(fixture["jobs"]["deny"])
        return not all(cargo_deny_contract(fixture).values())

    def deny_installer(job):
        return next(
            step for step in steps(job)
            if str(step.get("uses") or "").startswith("taiki-e/install-action@")
            and str((step.get("with") or {}).get("tool") or "").startswith("cargo-deny")
        )

    def deny_command(job):
        return next(step for step in steps(job) if "cargo deny" in str(step.get("run") or ""))

    deny_fixtures = (
        (
            "cargo-deny without checksum verification is rejected",
            lambda job: deny_installer(job)["with"].update(checksum=False),
        ),
        (
            "cargo-deny with a source-build fallback is rejected",
            lambda job: deny_installer(job)["with"].update(fallback="cargo"),
        ),
        (
            "an unreviewed cargo-deny version is rejected",
            lambda job: deny_installer(job)["with"].update(tool="cargo-deny@0.20.3"),
        ),
        (
            "the Docker-based cargo-deny action is rejected",
            lambda job: job["steps"].append({
                "uses": "EmbarkStudios/cargo-deny-action@fixture"
            }),
        ),
        (
            "a cargo-deny command without the lockfile flag is rejected",
            lambda job: deny_command(job).update(
                run=deny_command(job)["run"].replace(" --locked", "")
            ),
        ),
        (
            "a cargo-deny matrix missing a policy check is rejected",
            lambda job: job["strategy"]["matrix"].update(
                check=["advisories", "bans", "licenses"]
            ),
        ),
        (
            "a hardcoded cargo-deny policy subset is rejected",
            lambda job: deny_command(job).update(
                run=deny_command(job)["run"].replace("${{ matrix.check }}", "advisories")
            ),
        ),
    )
    for desc, mutate in deny_fixtures:
        emit(broken_deny(mutate), "self-test: {}".format(desc),
             "the cargo-deny wiring detector accepted a broken fixture")

    webview_build_task, webview_extension, pairing_frontend, pairing_wrapper = \
        ANDROID_WEBVIEW_SOURCES
    webview_contract_fixtures = (
        (
            "a renamed RustWebView extension file is rejected",
            webview_build_task.replace(
                "src/main/rust-webview-accessibility.kt.inc",
                "src/main/missing-accessibility.kt.inc",
            ),
            webview_extension,
            pairing_frontend,
            pairing_wrapper,
        ),
        (
            "an unexported RustWebView extension is rejected",
            webview_build_task.replace(
                "WRY_RUSTWEBVIEW_CLASS_EXTENSION",
                "WRY_UNUSED_CLASS_EXTENSION",
            ),
            webview_extension,
            pairing_frontend,
            pairing_wrapper,
        ),
        (
            "a deleted RustWebView extension is rejected",
            webview_build_task,
            "",
            pairing_frontend,
            pairing_wrapper,
        ),
        (
            "a RustWebView extension without the direct provider is rejected",
            webview_build_task,
            webview_extension.replace("super.getAccessibilityNodeProvider()", "null"),
            pairing_frontend,
            pairing_wrapper,
        ),
        (
            "a frontend and native pairing marker mismatch is rejected",
            webview_build_task,
            webview_extension,
            pairing_frontend.replace(
                "copypaste-pairing-dialog-open", "different-pairing-dialog"
            ),
            pairing_wrapper,
        ),
    )
    for desc, *fixture in webview_contract_fixtures:
        held, _ = android_webview_accessibility_contract(*fixture)
        emit(not held, "self-test: {}".format(desc),
             "the Android WebView provider detector accepted broken wiring")

    def windows_shard_fixture():
        def job(command, timeout=20, key="fixture"):
            return {
                "timeout-minutes": timeout,
                "steps": [
                    {"uses": "Swatinem/rust-cache@fixture",
                     "with": {"save-if": True, "shared-key": key}},
                    {"run": command},
                ],
            }
        return {
            "windows-clippy": job(
                "cargo +1.96 clippy --workspace --all-targets --locked -- -D warnings",
                key="clippy"),
            "windows-test-core": job(
                "cargo +1.96 test --locked -p copypaste-core -p copypaste-sensitive-rules",
                timeout=15, key="core"),
            "windows-test-services": job(
                "cargo +1.96 test --locked -p copypaste-p2p -p copypaste-cloud -p copypaste-daemon",
                timeout=15, key="services"),
            "windows-test-apps": job(
                "cargo +1.96 test --locked -p copypaste-ui -p copypaste-cli -p copypaste-ipc "
                "-p copypaste-runtime-log -p copypaste-fs -p copypaste-clock "
                "-p copypaste-feedback -p copypaste-retry",
                timeout=15, key="apps"),
            "windows-native-test": job(
                "pairing_presentation::windows::refusal_tests\n"
                "crypto::keystore::\ntest:native-parity", key="native"),
        }

    windows_fixture = windows_shard_fixture()
    windows_cache_fixture = windows_shard_fixture()
    windows_cache_fixture["windows-test-apps"]["steps"][0]["with"]["shared-key"] = "core"
    windows_coverage_fixture = windows_shard_fixture()
    windows_coverage_fixture["windows-test-apps"]["steps"][1]["run"] = \
        windows_coverage_fixture["windows-test-apps"]["steps"][1]["run"].replace(
            " -p copypaste-clock", "")
    windows_combined_fixture = windows_shard_fixture()
    windows_combined_fixture["windows-native-test"]["steps"][1]["run"] += \
        "\ncargo +1.96 test --workspace --locked"
    windows_budget_fixture = windows_shard_fixture()
    windows_budget_fixture["windows-test-core"]["timeout-minutes"] = 16
    for desc, held in (
        ("bounded package-complete Windows shards are accepted",
         windows_workspace_shards_hold(windows_fixture, WORKSPACE_PACKAGES, RUST_VERSION)),
        ("duplicate Windows cache writers are rejected",
         not windows_workspace_shards_hold(windows_cache_fixture, WORKSPACE_PACKAGES, RUST_VERSION)),
        ("reduced Windows workspace coverage is rejected",
         not windows_workspace_shards_hold(windows_coverage_fixture, WORKSPACE_PACKAGES, RUST_VERSION)),
        ("recombined Windows workspace work is rejected",
         not windows_workspace_shards_hold(windows_combined_fixture, WORKSPACE_PACKAGES, RUST_VERSION)),
        ("a Windows test shard above its target is rejected",
         not windows_workspace_shards_hold(windows_budget_fixture, WORKSPACE_PACKAGES, RUST_VERSION)),
    ):
        emit(held, "self-test: {}".format(desc),
             "the Windows workspace shard detector did not behave as stated")

    local_gate_source = pathlib.Path("scripts/prepush/wsl/verify.sh").read_text()
    ci_gate_fixture = {
        gate_id: {"steps": [{"run": "python3 scripts/ci/run-gates.py --gate {}".format(gate_id)}]}
        for gate_id in CI_GATES["profiles"]["linux-ci-mirror"]
    }
    missing_gate_fixture = copy.deepcopy(ci_gate_fixture)
    missing_gate_fixture.pop("file-size-budget")
    duplicate_gate_fixture = copy.deepcopy(ci_gate_fixture)
    duplicate_gate_fixture["duplicate"] = {
        "steps": [{"run": "python3 scripts/ci/run-gates.py --gate feature-ledger"}]
    }
    inert_gate_fixture = copy.deepcopy(ci_gate_fixture)
    inert_gate_fixture["feature-ledger"]["steps"][0]["run"] = \
        "echo python3 scripts/ci/run-gates.py --gate feature-ledger"
    advisory_registry = copy.deepcopy(CI_GATES)
    advisory_registry["gates"]["file-size-budget"]["commands"] = [
        ["bash", "scripts/check-file-size.sh", "500"]
    ]
    for desc, held in (
        ("the oversized-file fixture and enforcing gate reach both entry points",
         portable_gate_contract_holds(
             CI_GATES, "linux-ci-mirror", local_gate_source, ci_gate_fixture)),
        ("a CI gate missing from the local profile is rejected",
         not portable_gate_contract_holds(
             CI_GATES, "linux-ci-mirror", local_gate_source, missing_gate_fixture)),
        ("a duplicated CI gate is rejected",
         not portable_gate_contract_holds(
             CI_GATES, "linux-ci-mirror", local_gate_source, duplicate_gate_fixture)),
        ("an inert mention of a CI gate is rejected",
         not portable_gate_contract_holds(
             CI_GATES, "linux-ci-mirror", local_gate_source, inert_gate_fixture)),
        ("the advisory file-size checker cannot replace the enforcing gate",
         not portable_gate_contract_holds(
             advisory_registry, "linux-ci-mirror", local_gate_source, ci_gate_fixture)),
    ):
        emit(held, "self-test: {}".format(desc),
             "the portable gate registry detector did not behave as stated")

    stale_toolchain_fixture = copy.deepcopy(docs["ci.yml"])
    stale_toolchain_fixture["env"]["MSRV"] = "1.95"
    stale_toolchain_holds, _ = ci_rust_toolchain_holds(
        stale_toolchain_fixture, RUST_VERSION)
    emit(not stale_toolchain_holds,
         "self-test: a CI toolchain stale against Cargo rust-version is rejected",
         "the CI toolchain detector accepted a stale MSRV")

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

    def timeout_probe(timeout, reusable=False):
        job = {"uses": "./.github/workflows/other.yml"} if reusable else {
            "runs-on": "ubuntu-latest"
        }
        if timeout is not None:
            job["timeout-minutes"] = timeout
        return all(cond for cond, _, _ in
                   job_timeout_checks({"probe.yml": {"jobs": {"job": job}}}))

    for desc, held in (
        ("a 20-minute job budget is accepted", timeout_probe(20)),
        ("a shorter job budget is accepted", timeout_probe(7)),
        ("a missing job budget is rejected", not timeout_probe(None)),
        ("a 21-minute job budget is rejected", not timeout_probe(21)),
        ("a string job budget is rejected", not timeout_probe("20")),
        ("a reusable-workflow call is owned by its callee", timeout_probe(None, reusable=True)),
    ):
        emit(held, "self-test: {}".format(desc),
             "the job timeout detector did not behave as stated")

    current_nightly = docs.get("native-nightly.yml") or {}
    shared_group_nightly = copy.deepcopy(current_nightly)
    shared_group_nightly["concurrency"]["group"] = "native-nightly"
    cancelling_nightly = copy.deepcopy(current_nightly)
    cancelling_nightly["concurrency"]["cancel-in-progress"] = True
    missing_nightly_concurrency = copy.deepcopy(current_nightly)
    missing_nightly_concurrency.pop("concurrency", None)
    for desc, held in (
        ("the nightly workflow isolates scheduled and manual groups",
         nightly_concurrency_holds(current_nightly)),
        ("a shared nightly group is rejected",
         not nightly_concurrency_holds(shared_group_nightly)),
        ("cancelling nightly concurrency is rejected",
         not nightly_concurrency_holds(cancelling_nightly)),
        ("missing nightly concurrency is rejected",
         not nightly_concurrency_holds(missing_nightly_concurrency)),
    ):
        emit(held, "self-test: {}".format(desc),
             "the nightly concurrency detector did not behave as stated")

    def components_probe(components, run="cargo +1.96 test --workspace --locked"):
        setup = {"uses": "dtolnay/rust-toolchain@2c7215f132e9ebf062739d9130488b56d53c060c"}
        if components is not None:
            setup["with"] = {"toolchain": "1.96", "components": components}
        jobs = {"j": {"steps": [setup, {"run": run}]}}
        return all(cond for cond, _, _ in
                   toolchain_component_checks({"probe.yml": {"jobs": jobs}}, ["rustfmt"]))

    for desc, held in (
        ("a workspace test job installing rustfmt is accepted", components_probe("rustfmt")),
        ("clippy alongside rustfmt is accepted", components_probe("clippy, rustfmt")),
        ("a workspace test job installing nothing is rejected", not components_probe(None)),
        ("clippy without rustfmt is rejected", not components_probe("clippy")),
        ("a component the name merely contains is rejected", not components_probe("rustfmt-preview")),
        ("a job whose only cargo run is clippy is not asked",
         components_probe(None, "cargo +1.96 clippy --workspace --all-targets --locked")),
        ("a single-crate test run is not asked",
         components_probe(None, "cargo +1.96 test -p copypaste-core --locked")),
    ):
        emit(held, "self-test: {}".format(desc),
             "the toolchain component detector did not behave as stated")

    def nightly_probe(name, gate, matrix=None, uses="./.github/workflows/android-emulator.yml"):
        job = {"uses": uses}
        if gate is not None:
            job["if"] = "github.event_name == '{}'".format(gate)
        if matrix is not None:
            job["strategy"] = {"fail-fast": False, "matrix": {"api-level": matrix}}
        return {name: job}

    def sched(**kw):
        return scheduled_sweep_is_single(nightly_probe("android-nightly", **kw))

    def disp(**kw):
        return dispatch_spot_check_holds(nightly_probe("android-dispatch", **kw))

    for desc, held in (
        ("an unmatrixed scheduled call is accepted", sched(gate="schedule")),
        ("a matrixed scheduled call is rejected", not sched(gate="schedule", matrix=[34, 36])),
        ("a scheduled call left on the dispatch gate is rejected", not sched(gate="workflow_dispatch")),
        ("an ungated scheduled call is rejected", not sched(gate=None)),
        ("a scheduled call to another workflow is rejected",
         not sched(gate="schedule", uses="./.github/workflows/ci.yml")),
        ("a negated scheduled gate is rejected",
         not scheduled_sweep_is_single(
             {"android-nightly": {"uses": "./.github/workflows/android-emulator.yml",
                                  "if": "github.event_name != 'schedule'"}})),
        ("a renamed scheduled job is rejected", not scheduled_sweep_is_single({})),
        ("the API 34/36 dispatch matrix is accepted",
         disp(gate="workflow_dispatch", matrix=[34, 36])),
        ("a dispatch matrix missing a leg is rejected", not disp(gate="workflow_dispatch", matrix=[34])),
        ("an unmatrixed dispatch call is rejected", not disp(gate="workflow_dispatch")),
        ("a renamed dispatch job is rejected", not dispatch_spot_check_holds({})),
    ):
        emit(held, "self-test: {}".format(desc),
             "the nightly Android call detector did not behave as stated")

    matching_paths = {
        True: {
            "push": {"paths": ["crates/copypaste-ui/src/**", "Cargo.toml"]},
            "pull_request": {"paths": ["crates/copypaste-ui/src/**", "Cargo.toml"]},
        }
    }
    reordered_paths = copy.deepcopy(matching_paths)
    reordered_paths[True]["pull_request"]["paths"].reverse()
    missing_paths = copy.deepcopy(matching_paths)
    del missing_paths[True]["pull_request"]["paths"]
    current_branch = {"ci.yml": {True: {"push": {"branches": ["main"]}}}}
    retired_branch = copy.deepcopy(current_branch)
    retired_branch["ci.yml"][True]["push"]["branches"].append("v2-main")
    for desc, held in (
        ("identical push and pull-request path filters are accepted",
         matching_event_path_filters(matching_paths)),
        ("reordered push and pull-request path filters are rejected",
         not matching_event_path_filters(reordered_paths)),
        ("a missing pull-request path filter is rejected",
         not matching_event_path_filters(missing_paths)),
        ("the current push branch is accepted",
         not retired_push_branch_violations(current_branch, "v2-main")),
        ("the retired push branch is rejected",
         retired_push_branch_violations(retired_branch, "v2-main") == ["ci.yml"]),
    ):
        emit(held, "self-test: {}".format(desc),
             "the workflow path-filter detector did not behave as stated")

    abi_defaults = (
        "TRIPLES=(aarch64-linux-android armv7-linux-androideabi "
        "i686-linux-android x86_64-linux-android)\n"
    )

    def abi_link_probe(triples, ndk_source=abi_defaults, command=None):
        command = command or './scripts/release/android-link-abis.sh "${{ matrix.triple }}"'
        return android_link_abi_matrix_holds(
            {
                "strategy": {"matrix": {"triple": triples}},
                "steps": [
                    {
                        "uses": "dtolnay/rust-toolchain@fixture",
                        "with": {"targets": "${{ matrix.triple }}"},
                    },
                    {
                        "uses": "Swatinem/rust-cache@fixture",
                        "with": {"shared-key": "android-link-${{ matrix.triple }}"},
                    },
                    {"run": command},
                ],
            },
            abi_defaults,
            ndk_source,
        )

    abi_targets = shell_array(abi_defaults, "TRIPLES")
    for desc, held in (
        ("the exact Android ABI link matrix is accepted", abi_link_probe(abi_targets)),
        ("an ABI matrix deletion is rejected", not abi_link_probe(abi_targets[:-1])),
        ("an ABI matrix substitution is rejected",
         not abi_link_probe(abi_targets[:-1] + ["riscv64-linux-android"])),
        ("an ABI matrix duplication is rejected",
         not abi_link_probe(abi_targets + [abi_targets[-1]])),
        ("a batched ABI link invocation is rejected",
         not abi_link_probe(abi_targets, command="./scripts/release/android-link-abis.sh")),
        ("NDK mappings cannot diverge from link defaults",
         not abi_link_probe(abi_targets, ndk_source="TRIPLES=(aarch64-linux-android)\n")),
    ):
        emit(held, "self-test: {}".format(desc),
             "the Android ABI link matrix detector did not behave as stated")

    unsafe_android_transfer = {
        "shard": {
            "steps": [{
                "uses": "actions/upload-artifact@fixture",
                "with": {
                    "name": "android-release-scaffold",
                    "path": "gen/android/tauri.settings.gradle",
                },
            }]
        }
    }
    safe_android_transfer = {
        "shard": {
            "steps": [{
                "uses": "actions/upload-artifact@fixture",
                "with": {
                    "name": "android-native-aarch64",
                    "path": "libcopypaste_ui_lib.so",
                },
            }]
        }
    }
    opaque_android_transfer = {
        "shard": {
            "steps": [{
                "uses": "actions/upload-artifact@fixture",
                "with": {
                    "name": "android-generated",
                    "path": "crates/copypaste-ui/src-tauri/gen/android",
                },
            }]
        }
    }
    for desc, held in (
        ("runner-local Android settings are rejected as artifacts",
         bool(android_runner_local_artifact_transfers(unsafe_android_transfer))),
        ("an opaque generated Android tree artifact is rejected",
         bool(android_runner_local_artifact_transfers(opaque_android_transfer))),
        ("Android native libraries remain transferable",
         not android_runner_local_artifact_transfers(safe_android_transfer)),
    ):
        emit(held, "self-test: {}".format(desc),
             "the Android machine-local artifact detector did not behave as stated")

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

    guarded = """run_with_android_screencap_timeout() {
    timeout --foreground 15s "$@"
}
bounded_adb() {
    run_with_android_screencap_timeout adb "$@"
}
targeted_adb() {
    bounded_adb -s "$serial" "$@"
}
targeted_adb "$serial" shell dumpsys power
"""
    for desc, held in (
        ("targeted bounded adb is accepted", not adb_guard_violations(guarded, 1)),
        ("a raw adb call is rejected", bool(adb_guard_violations(guarded + "adb shell id\n", 1))),
        ("an unbounded targeted adb call is rejected",
         bool(adb_guard_violations(guarded + "bounded_adb shell id\n", 1))),
        ("a targeted wrapper without a serial is rejected",
         bool(adb_guard_violations(guarded + "targeted_adb -s shell id\n", 1))),
    ):
        emit(held, "self-test: {}".format(desc), "the adb structure detector did not reject the fixture")

# The plain run stays exit 0 so check.sh can enumerate every PASS|FAIL line.
sys.exit(1 if (SELF_TEST or STRICT) and SELF_TEST_FAILURES else 0)
