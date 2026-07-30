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

# --- release.yml shell discipline ------------------------------------------
for jn, j in (docs["release.yml"].get("jobs") or {}).items():
    for i, s in enumerate(steps(j)):
        body = s.get("run")
        # A one-command step gains nothing: GitHub already runs `bash -e`. The
        # rule is for multi-line blocks, where -u and -o pipefail are the ones
        # that matter.
        if not body or len(body.strip().splitlines()) < 2:
            continue
        rec(body.lstrip().startswith("set -euo pipefail"),
            "release.yml: {} step {} opens with set -euo pipefail".format(jn, i),
            "opens with: {}".format(body.lstrip().splitlines()[0][:60]))

sys.exit(0)
