import json
import subprocess


def workspace_contract(root):
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    metadata = json.loads(result.stdout)
    members = set(metadata["workspace_members"])
    packages = {
        package["name"] for package in metadata["packages"] if package["id"] in members
    }
    versions = {
        package.get("rust_version")
        for package in metadata["packages"]
        if package["id"] in members
    }
    if not packages or None in versions or len(versions) != 1:
        raise ValueError("workspace metadata has no single inherited rust-version")
    return packages, versions.pop()


def load_registry(root):
    registry = json.loads((root / "config/ci-gates.json").read_text())
    if set(registry) != {"schema", "gates", "profiles"} or registry["schema"] != 1:
        raise ValueError("ci-gates.json has an unsupported shape or schema")
    gates = registry["gates"]
    if not isinstance(gates, dict) or not gates:
        raise ValueError("ci-gates.json must declare gates")
    for gate_id, gate in gates.items():
        if set(gate) != {"label", "commands"}:
            raise ValueError("gate {!r} has unsupported fields".format(gate_id))
        commands = gate["commands"]
        if not isinstance(gate["label"], str) or not gate["label"] or not commands:
            raise ValueError("gate {!r} needs a label and commands".format(gate_id))
        if any(
            not isinstance(command, list)
            or not command
            or any(not isinstance(word, str) or not word for word in command)
            for command in commands
        ):
            raise ValueError("gate {!r} has an invalid command".format(gate_id))
    for profile_id, profile in registry["profiles"].items():
        if not isinstance(profile, list) or len(profile) != len(set(profile)):
            raise ValueError("profile {!r} must contain unique gate IDs".format(profile_id))
        unknown = set(profile) - set(gates)
        if unknown:
            raise ValueError(
                "profile {!r} names unknown gates: {}".format(profile_id, sorted(unknown))
            )
    return registry
