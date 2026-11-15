"""Generate Linux build, qualification and compatibility views from one contract."""

import argparse
import json

from linux_release_contract import APPIMAGETOOL, TARGETS


def matrix(tier: str, purpose: str) -> dict:
    targets = [target for target in TARGETS if tier == "all" or target["tier"] == tier]
    if purpose in {"build", "supplemental-build"}:
        stable_architectures = {
            target["architecture"] for target in TARGETS if target["tier"] == "stable"
        }
        rows = {}
        for target in targets:
            architecture = target["architecture"]
            if purpose == "supplemental-build" and architecture in stable_architectures:
                continue
            runner = target["test_environment"]["runner"]
            if architecture in rows and rows[architecture]["runner"] != runner:
                raise ValueError("One architecture has conflicting build runners")
            rows[architecture] = {
                "architecture": architecture,
                "runner": runner,
                "appimagetool_url": APPIMAGETOOL[architecture]["url"],
                "appimagetool_sha256": APPIMAGETOOL[architecture]["sha256"],
                "formats": ",".join(sorted({format_name for row in targets
                                            if row["architecture"] == architecture
                                            for format_name in row["packaging"]})),
            }
        return {"include": list(rows.values())}
    if purpose == "qualification":
        return {"include": [
            {
                "architecture": target["architecture"],
                "runner": target["test_environment"]["runner"],
                "desktop": target["desktop"],
                "session": target["session"],
                "target_id": target["id"],
                "compositor_artifact": target["compositor_artifact"],
            }
            for target in targets if target["native_qualification"]
        ]}
    raise ValueError(f"Unknown matrix purpose: {purpose}")


def table() -> str:
    lines = [
        "| Distribution | Desktop | Session | Architecture | Packages | Tier | Native qualification |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for target in TARGETS:
        lines.append("| " + " | ".join((
            f"{target['distribution']} {target['version']}",
            f"{target['desktop']} {target['desktop_version']}",
            target["session"], target["architecture"],
            ", ".join(target["packaging"]), target["tier"],
            "required" if target["native_qualification"] else "not yet qualified",
        )) + " |")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("view", choices=("matrix", "table"))
    parser.add_argument("--tier", choices=("stable", "experimental", "all"), default="stable")
    parser.add_argument("--purpose", choices=("build", "supplemental-build", "qualification"), default="qualification")
    args = parser.parse_args()
    print(json.dumps(matrix(args.tier, args.purpose), separators=(",", ":"))
          if args.view == "matrix" else table())


if __name__ == "__main__":
    main()
