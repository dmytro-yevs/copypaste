#!/usr/bin/env python3
import argparse
import pathlib
import subprocess
import sys

from gate_registry import load_registry, workspace_contract


ROOT = pathlib.Path(__file__).resolve().parents[2]


def run_gate(gate_id, gate, rust_version):
    print("\n################ {} ({})".format(gate["label"], gate_id), flush=True)
    for command in gate["commands"]:
        resolved = [word.format(rust_version=rust_version) for word in command]
        print("+ {}".format(" ".join(resolved)), flush=True)
        result = subprocess.run(resolved, cwd=ROOT)
        if result.returncode:
            return result.returncode
    return 0


def main():
    parser = argparse.ArgumentParser()
    choice = parser.add_mutually_exclusive_group(required=True)
    choice.add_argument("--gate", action="append")
    choice.add_argument("--profile")
    choice.add_argument("--check", action="store_true")
    parser.add_argument("--expect-command")
    args = parser.parse_args()

    try:
        registry = load_registry(ROOT)
        if args.check:
            workspace_contract(ROOT)
            return 0
        gate_ids = args.gate or registry["profiles"].get(args.profile)
        if gate_ids is None:
            raise ValueError("unknown gate profile {!r}".format(args.profile))
        unknown = set(gate_ids) - set(registry["gates"])
        if unknown:
            raise ValueError("unknown gate IDs: {}".format(sorted(unknown)))
        if args.expect_command:
            words = {
                word
                for gate_id in gate_ids
                for command in registry["gates"][gate_id]["commands"]
                for word in command
            }
            if args.expect_command not in words:
                raise ValueError(
                    "selected gates do not execute {!r}".format(args.expect_command)
                )
        needs_rust = any(
            "{rust_version}" in word
            for gate_id in gate_ids
            for command in registry["gates"][gate_id]["commands"]
            for word in command
        )
        rust_version = workspace_contract(ROOT)[1] if needs_rust else ""
    except (KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print("ci gate registry: {}".format(error), file=sys.stderr)
        return 2

    failed = []
    for gate_id in gate_ids:
        if run_gate(gate_id, registry["gates"][gate_id], rust_version):
            failed.append(gate_id)
    print("\n================ CI GATE SUMMARY")
    for gate_id in gate_ids:
        print("{}  {}".format("FAIL" if gate_id in failed else "PASS", gate_id))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
