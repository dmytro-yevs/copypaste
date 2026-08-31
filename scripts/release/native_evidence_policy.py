#!/usr/bin/env python3
import argparse
import copy
import json
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]
POLICY_PATH = ROOT / "config" / "native-evidence-policy.json"
SCHEMA_PATH = ROOT / "crates" / "copypaste-ui" / "scripts" / "native-parity-evidence.schema.json"


def load_policy():
    policy = json.loads(POLICY_PATH.read_text(encoding="utf-8"))
    environments = policy.get("environments")
    artifact_kinds = policy.get("artifact_kinds")
    platforms = policy.get("platforms")
    if (
        set(policy) != {
            "schema_version", "receipt_schema_version", "environments",
            "artifact_kinds", "platforms",
        }
        or policy.get("schema_version") != 1
        or policy.get("receipt_schema_version") != 2
        or not isinstance(environments, list)
        or not environments
        or any(not isinstance(value, str) or not value for value in environments)
        or len(environments) != len(set(environments))
        or not isinstance(artifact_kinds, list)
        or not artifact_kinds
        or any(not isinstance(value, str) or not value for value in artifact_kinds)
        or len(artifact_kinds) != len(set(artifact_kinds))
        or not isinstance(platforms, dict)
        or set(platforms) != {"macos", "android", "windows"}
    ):
        raise ValueError("native evidence policy has an invalid top-level contract")
    release_artifacts = set()
    for platform, requirement in platforms.items():
        artifacts = requirement.get("artifacts") or {}
        required = artifacts.get("required")
        optional = artifacts.get("optional")
        repeatable = artifacts.get("repeatable")
        assertions = requirement.get("assertions")
        release_artifact = requirement.get("release_artifact")
        if (
            set(requirement) != {
                "environment", "scenario", "budget_ms", "assertions", "artifacts",
                "feature_state_artifacts", "release_artifact",
            }
            or set(artifacts) != {"required", "optional", "repeatable"}
            or requirement.get("environment") not in environments
            or not isinstance(requirement.get("scenario"), str)
            or not requirement["scenario"]
            or not isinstance(requirement.get("budget_ms"), int)
            or isinstance(requirement["budget_ms"], bool)
            or requirement["budget_ms"] < 1
            or not isinstance(assertions, list)
            or not assertions
            or len(assertions) != len(set(assertions))
            or any(not isinstance(value, str) or not value for value in assertions)
            or any(not isinstance(values, list) for values in (required, optional, repeatable))
            or set(required) & set(optional)
            or not set(required) | set(optional) <= set(artifact_kinds)
            or not set(repeatable) <= set(required) | set(optional)
            or not isinstance(requirement.get("feature_state_artifacts"), list)
            or len(requirement["feature_state_artifacts"]) != len(set(requirement["feature_state_artifacts"]))
            or not set(requirement["feature_state_artifacts"]) <= {"screenshot", "accessibility"}
            or not set(requirement["feature_state_artifacts"]) <= set(required) | set(optional)
            or not isinstance(release_artifact, str)
            or not release_artifact
            or release_artifact in release_artifacts
        ):
            raise ValueError(f"native evidence policy for {platform} is invalid")
        release_artifacts.add(release_artifact)
    return policy


def schema_document(policy):
    platforms = policy["platforms"]
    assertions = list(dict.fromkeys(
        value for item in platforms.values() for value in item["assertions"]
    ))
    scenarios = sorted({item["scenario"] for item in platforms.values()})
    conditions = []
    for platform, requirement in platforms.items():
        allowed_kinds = requirement["artifacts"]["required"] + requirement["artifacts"]["optional"]
        platform_properties = {
            "environment": {"const": requirement["environment"]},
            "scenario": {
                "type": "object",
                "properties": {
                    "name": {"const": requirement["scenario"]},
                    "budget_ms": {"const": requirement["budget_ms"]},
                },
            },
            "assertions": {
                "type": "array",
                "minItems": len(requirement["assertions"]),
                "maxItems": len(requirement["assertions"]),
                "allOf": [
                    {"contains": {"const": assertion}}
                    for assertion in requirement["assertions"]
                ],
            },
            "artifacts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {"kind": {"enum": allowed_kinds}},
                },
            },
        }
        feature_state_artifacts = requirement["feature_state_artifacts"]
        if feature_state_artifacts:
            platform_properties["feature_states"] = {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": feature_state_artifacts,
                    "properties": {
                        kind: {} for kind in feature_state_artifacts
                    },
                },
            }
        conditions.append({
            "if": {"type": "object", "properties": {"platform": {"const": platform}}},
            "then": {
                "type": "object",
                "properties": platform_properties,
            },
        })
    artifact_reference = {
        "type": "object", "additionalProperties": False,
        "required": ["path", "sha256", "bytes"],
        "properties": {
            "path": {
                "type": "string", "minLength": 1, "maxLength": 240,
                "pattern": "^[A-Za-z0-9][A-Za-z0-9._-]*(?:/[A-Za-z0-9][A-Za-z0-9._-]*)*$",
            },
            "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
            "bytes": {"type": "integer", "minimum": 1},
        },
    }
    return {
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": "https://copypaste.invalid/native-parity-evidence.schema.json",
        "type": "object",
        "additionalProperties": False,
        "required": [
            "schema_version", "platform", "environment", "os_version", "architecture",
            "source", "scenario", "assertions", "artifacts", "qualified_artifact",
        ],
        "properties": {
            "schema_version": {"const": policy["receipt_schema_version"]},
            "platform": {"enum": list(platforms)},
            "environment": {"enum": policy["environments"]},
            "os_version": {"type": "string", "minLength": 1, "maxLength": 160},
            "architecture": {"type": "string", "minLength": 1, "maxLength": 80},
            "source": {
                "type": "object", "additionalProperties": False,
                "required": ["commit", "run_id"],
                "properties": {
                    "commit": {"type": "string", "pattern": "^[0-9a-f]{40}$"},
                    "run_id": {"type": "string", "pattern": "^[A-Za-z0-9][A-Za-z0-9._-]{0,119}$"},
                },
            },
            "scenario": {
                "type": "object", "additionalProperties": False,
                "required": ["name", "elapsed_ms", "budget_ms"],
                "properties": {
                    "name": {"enum": scenarios},
                    "elapsed_ms": {"type": "integer", "minimum": 0},
                    "budget_ms": {"type": "integer", "minimum": 1},
                },
            },
            "assertions": {
                "type": "array", "minItems": 1, "uniqueItems": True,
                "items": {"enum": assertions},
            },
            "artifacts": {
                "type": "array", "minItems": 1,
                "items": {
                    "type": "object", "additionalProperties": False,
                    "required": ["kind", "path", "sha256", "bytes"],
                    "properties": {
                        "kind": {"enum": policy["artifact_kinds"]},
                        "path": {
                            "type": "string", "minLength": 1, "maxLength": 240,
                            "pattern": "^[A-Za-z0-9][A-Za-z0-9._-]*(?:/[A-Za-z0-9][A-Za-z0-9._-]*)*$",
                        },
                        "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                        "bytes": {"type": "integer", "minimum": 1},
                    },
                },
            },
            "qualified_artifact": {
                "type": "object", "additionalProperties": False,
                "required": ["name", "sha256", "bytes"],
                "properties": {
                    "name": {
                        "type": "string", "minLength": 1, "maxLength": 240,
                        "pattern": "^[A-Za-z0-9][A-Za-z0-9._-]*$",
                    },
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "bytes": {"type": "integer", "minimum": 1},
                },
            },
            "feature_states": {
                "type": "array", "minItems": 1, "uniqueItems": True,
                "items": {
                    "type": "object", "additionalProperties": False,
                    "required": ["feature_id", "state"],
                    "properties": {
                        "feature_id": {
                            "type": "string", "pattern": "^[a-z0-9][a-z0-9_-]*$",
                        },
                        "state": {
                            "type": "string", "pattern": "^[a-z0-9][a-z0-9_-]*$",
                        },
                        "screenshot": copy.deepcopy(artifact_reference),
                        "accessibility": copy.deepcopy(artifact_reference),
                    },
                },
            },
        },
        "allOf": conditions,
    }


def serialized_schema(policy):
    return json.dumps(schema_document(policy), indent=2, ensure_ascii=False) + "\n"


def main():
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("check-schema")
    subparsers.add_parser("render-schema")
    subparsers.add_parser("self-test")
    value = subparsers.add_parser("value")
    value.add_argument("--platform", choices=("macos", "android", "windows"), required=True)
    value.add_argument(
        "--field",
        choices=("environment", "scenario", "budget_ms", "release_artifact"),
        required=True,
    )
    args = parser.parse_args()
    try:
        policy = load_policy()
    except (OSError, ValueError, json.JSONDecodeError) as error:
        raise SystemExit(f"native-evidence-policy: {error}") from None
    if args.command == "value":
        print(policy["platforms"][args.platform][args.field])
        return
    if args.command == "render-schema":
        print(serialized_schema(policy), end="")
        return
    if args.command == "self-test":
        projected = schema_document(policy)
        mutated = copy.deepcopy(policy)
        mutated["platforms"]["android"]["budget_ms"] += 1
        if projected == schema_document(mutated):
            raise SystemExit("native-evidence-policy: stale schema fixture passed")
        mutated = copy.deepcopy(policy)
        mutated["platforms"]["android"]["environment"] = "emulator"
        if projected == schema_document(mutated):
            raise SystemExit("native-evidence-policy: stale Android environment fixture passed")
        mutated = copy.deepcopy(policy)
        mutated["platforms"]["android"]["feature_state_artifacts"] = []
        if projected == schema_document(mutated):
            raise SystemExit("native-evidence-policy: stale feature-state binding fixture passed")
        print("native evidence policy self-test passed")
        return
    try:
        current = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        raise SystemExit("native-evidence-policy: receipt schema is unavailable") from None
    if current != schema_document(policy):
        raise SystemExit("native-evidence-policy: receipt schema is stale")
    print("native evidence policy projections are current")


if __name__ == "__main__":
    main()
