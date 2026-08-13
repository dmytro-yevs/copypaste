#!/usr/bin/env python3
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
MARKERS = re.compile(r"\b(?:TO" + r"DO|FIX" + r"ME|HA" + r"CK|X" + r"XX)\b", re.IGNORECASE)
LINK = re.compile(r"\[[^]]*\]\(([^)]+)\)")

IPC_LIB = ROOT / "crates" / "copypaste-ipc" / "src" / "lib.rs"
PROTOCOL_RE = re.compile(r"pub\s+const\s+PROTOCOL_VERSION\s*:\s*u32\s*=\s*(\d+)\s*;")

PROTOCOL_DOC_SITES = {
    "docs/rewrite/target-architecture.md": re.compile(
        r"`PROTOCOL_VERSION`\s+is\s+`(\d+)`"
    ),
    "docs/rewrite/port-manifest/06-ui-behaviour.md": re.compile(
        r"`CURRENT_PROTOCOL_VERSION\s*=\s*(\d+)`"
    ),
}

README = "README.md"
CONTENT_TYPE_RS = ROOT / "crates" / "copypaste-ipc" / "src" / "content_type.rs"
CONTENT_CONST_RE = re.compile(r'pub\s+const\s+([A-Z][A-Z0-9_]*)\s*:\s*&str\s*=\s*"([^"]*)"\s*;')
KNOWN_RE = re.compile(r"pub\s+const\s+KNOWN\s*:\s*&\[&str\]\s*=\s*&\[([^\]]*)\]\s*;")

PRODUCT_LIMITS_HEADING = "### Product limits"
# Each pattern is one phrasing of the blanket claim that the product handles no
# non-text clipboard content. `format.rs` really does capture plain text only,
# so a *scoped* statement about native capture must not use these forms.
TEXT_ONLY_CLAIMS = (
    re.compile(r"(?i)\b(?:is|are|remains?|stays?)\s+text[-\s]only\b"),
    re.compile(
        r"(?i)\bdoes\s+not\s+(?:capture|store|support|handle|keep)\s+"
        r"[^.\n]*\b(?:images|files|rich\s+text)\b"
    ),
    re.compile(r"(?i)\bplain[-\s]text[-\s]only\s+(?:clipboard|manager|product|app)\b"),
)

CLOUD_LINE_RE = re.compile(r"(?i)supabase|cloud\s+sync")
# Two layers are established, and the copy has to keep them apart: the demo runs
# against a stub, while the release gate runs against a genuine Supabase stack
# that is local and disposable. Neither reaches a hosted project, so a sentence
# that puts one behind our checks is the overclaim to catch.
CLOUD_OVERCLAIMS = (
    re.compile(
        r"(?i)\bagainst\s+(?:a|an|the|our|one)?\s*"
        r"(?:real|live|deployed|production|hosted)\s+(?:\w+\s+){0,2}supabase\b"
    ),
    re.compile(
        r"(?i)\b(?:real|live|deployed|production|hosted)\s+(?:\w+\s+){0,2}"
        r"supabase\s+(?:project|deployment|instance|backend|environment|org)\b"
    ),
    re.compile(r"(?i)\bsupabase\s*(?:\.\s*com|cloud)\b"),
)
CLOUD_NEGATION_RE = re.compile(r"(?i)\b(?:no|not|never|nothing|none|unverified|stub)\b")
# Anchored on repo paths and the workflow job id rather than on a historical
# absolute: "nothing has ever spoken to a real project" was itself false, and a
# guard that demands a sentence keeps that sentence true forever by fiat.
CLOUD_REQUIRED = (
    ("names the local stub script", re.compile(r"scripts/cloud-stub\.py")),
    ("calls the demo backend a local stub", re.compile(r"(?i)\blocal\s+stub\b")),
    ("says no workflow runs the cloud demo", re.compile(r"(?i)\bno\s+workflow\s+runs\s+it\b")),
    ("names the real-Supabase gate script", re.compile(r"supabase/tests/real-supabase\.sh")),
    ("names the release job that runs the gate", re.compile(r"\bsupabase-gate\b")),
    (
        "calls that gate's stack local and disposable",
        re.compile(r"(?i)\bdisposable\s+local\s+supabase\s+stack\b"),
    ),
    (
        "keeps the Works row pointing at that caveat",
        re.compile(r"(?i)\bWired\s+to\s+the\s+daemon\s+and\s+the\s+CLI\b[^|\n]*\bbut\s+see\s+below\b"),
    ),
)
CLOUD_ANCHORS = (
    ("supabase/tests/real-supabase.sh", None),
    (".github/workflows/release.yml", re.compile(r"(?m)^\s*supabase-gate:")),
)


def ipc_protocol_version():
    text = IPC_LIB.read_text(encoding="utf-8")
    m = PROTOCOL_RE.search(text)
    if not m:
        raise SystemExit(f"Cannot parse PROTOCOL_VERSION from {IPC_LIB}")
    return int(m.group(1))


def check_protocol_docs(runtime_version, *, inject=None):
    """Verify docs cite the runtime protocol version.

    *inject* replaces file content for the self-test.
    """
    errs = []
    for rel_path, pattern in PROTOCOL_DOC_SITES.items():
        if inject and rel_path in inject:
            text = inject[rel_path]
        else:
            text = (ROOT / rel_path).read_text(encoding="utf-8-sig")
        m = pattern.search(text)
        if not m:
            errs.append(f"{rel_path}: protocol-version reference not found")
        elif int(m.group(1)) != runtime_version:
            errs.append(
                f"{rel_path}: protocol version {m.group(1)} does not match "
                f"runtime PROTOCOL_VERSION ({runtime_version})"
            )
    return errs


def non_plain_text_types():
    """Every content type `KNOWN` names that is not plain text.

    Parsed the same narrow way `PROTOCOL_RE` parses the protocol version: two
    anchored regexes over one file, not a Rust parser.
    """
    text = CONTENT_TYPE_RS.read_text(encoding="utf-8")
    values = dict(CONTENT_CONST_RE.findall(text))
    m = KNOWN_RE.search(text)
    if not m:
        raise SystemExit(f"Cannot parse KNOWN from {CONTENT_TYPE_RS}")
    kinds = []
    for name in (part.strip() for part in m.group(1).split(",")):
        if not name:
            continue
        if name not in values:
            raise SystemExit(f"KNOWN names {name}, which {CONTENT_TYPE_RS} does not define")
        if values[name] != values.get("TEXT"):
            kinds.append(values[name])
    return kinds


def readme_text(inject):
    if inject and README in inject:
        return inject[README]
    return (ROOT / README).read_text(encoding="utf-8-sig")


def check_product_limits(non_plain_types, *, inject=None):
    """Reject a blanket text-only claim while non-text types are shipped.

    *inject* replaces file content for the self-test.
    """
    lines = readme_text(inject).splitlines()
    starts = [i for i, line in enumerate(lines) if line.strip() == PRODUCT_LIMITS_HEADING]
    if not starts:
        return [f"{README}: '{PRODUCT_LIMITS_HEADING}' section not found"]
    if not non_plain_types:
        return []
    start = starts[0]
    end = next(
        (i for i in range(start + 1, len(lines)) if lines[i].startswith("#")),
        len(lines),
    )
    errs = []
    for number in range(start + 1, end):
        for pattern in TEXT_ONLY_CLAIMS:
            if pattern.search(lines[number]):
                errs.append(
                    f"{README}:{number + 1}: product limits claim CopyPaste handles no "
                    f"non-text content, but content_type.rs KNOWN names "
                    f"{', '.join(non_plain_types)}: {lines[number].strip()}"
                )
                break
    return errs


def check_cloud_claims(*, inject=None):
    """Hold the cloud copy to both validated layers and to no more than them.

    The demo's stub and the release gate's disposable local stack are separate
    claims, and dropping either one leaves the row misleading in a different
    direction. *inject* replaces file content for the self-test.
    """
    text = readme_text(inject)
    errs = [
        f"{README}: cloud copy no longer {label}"
        for label, pattern in CLOUD_REQUIRED
        if not pattern.search(text)
    ]
    for relative, pattern in CLOUD_ANCHORS:
        path = ROOT / relative
        if not path.is_file():
            errs.append(f"{README}: cloud copy cites {relative}, which does not exist")
        elif pattern and not pattern.search(path.read_text(encoding="utf-8")):
            errs.append(f"{README}: cloud copy cites the supabase-gate job, absent from {relative}")
    for number, line in enumerate(text.splitlines(), 1):
        if not CLOUD_LINE_RE.search(line):
            continue
        for sentence in re.split(r"(?<=[.;])\s+", line):
            if CLOUD_NEGATION_RE.search(sentence):
                continue
            if any(pattern.search(sentence) for pattern in CLOUD_OVERCLAIMS):
                errs.append(
                    f"{README}:{number}: cloud copy claims a hosted Supabase project; the gate "
                    f"runs a disposable local stack: {sentence.strip()}"
                )
                break
    return errs


def tracked_files():
    roots = (ROOT / ".github" / "workflows", ROOT / "scripts", ROOT / "docs")
    suffixes = {".md", ".py", ".sh", ".yml", ".yaml"}
    for root in roots:
        for path in root.rglob("*"):
            if path.is_file() and path.suffix in suffixes:
                yield path


def main():
    errors = []

    pv = ipc_protocol_version()
    errors.extend(check_protocol_docs(pv))
    errors.extend(check_product_limits(non_plain_text_types()))
    errors.extend(check_cloud_claims())

    for path in tracked_files():
        relative = path.relative_to(ROOT).as_posix()
        text = path.read_text(encoding="utf-8-sig")
        historical = relative.startswith("docs/rewrite/port-manifest/")
        # The scripts that detect these markers have to spell them out.
        marker_example = relative in {
            "scripts/check-commit-msg.sh",
            "scripts/check-feature-ledger.py",
        }
        if not historical and not marker_example:
            for number, line in enumerate(text.splitlines(), 1):
                if MARKERS.search(line):
                    errors.append(f"{relative}:{number}: unfinished-work marker: {line.strip()}")
        if path.suffix != ".md":
            continue
        for match in LINK.finditer(text):
            target = match.group(1).split("#", 1)[0]
            if not target or "://" in target or target.startswith("mailto:"):
                continue
            if not (path.parent / target).resolve().exists():
                number = text.count("\n", 0, match.start()) + 1
                errors.append(f"{relative}:{number}: missing local link target: {target}")

    if errors:
        print("\n".join(errors))
        sys.exit(1)
    print("Documentation links and unfinished-work markers are clean.")


if __name__ == "__main__":
    main()
