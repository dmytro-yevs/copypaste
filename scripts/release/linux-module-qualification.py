#!/usr/bin/env python3
"""Qualify first-party Linux modules through the installed GUI daemon IPC."""

import hashlib
import importlib.util
import json
import math
import os
import shutil
import socket
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CATALOG_SPEC = importlib.util.spec_from_file_location(
    "copypaste_module_catalog", ROOT / "scripts/modules/catalog.py",
)
catalog = importlib.util.module_from_spec(CATALOG_SPEC)
assert CATALOG_SPEC.loader is not None
CATALOG_SPEC.loader.exec_module(catalog)

MODULES = (
    "copypaste.ocr",
    "copypaste.semantic-search",
    "copypaste.supabase",
)


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def regular_child(root, relative):
    if not isinstance(relative, str) or not relative or "\\" in relative:
        raise ValueError("module fixture path is invalid")
    path = root / relative
    try:
        resolved = path.resolve(strict=True)
    except FileNotFoundError as error:
        raise ValueError(f"module fixture is missing: {relative}") from error
    if path.is_symlink() or not resolved.is_file() or root.resolve() not in resolved.parents:
        raise ValueError(f"module fixture is unsafe: {relative}")
    return resolved


def receipt_for(package, module_id, architecture, app_version):
    receipt_path = package.with_name(package.name + ".receipt.json")
    if not receipt_path.is_file() or receipt_path.is_symlink():
        raise ValueError(f"signed module receipt is missing: {package.name}")
    try:
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError(f"signed module receipt is not JSON: {package.name}") from error
    target = {"platform": "linux", "architecture": architecture}
    if (
        receipt.get("module_id") != module_id
        or receipt.get("target") != target
        or receipt.get("app_version") != app_version
        or receipt.get("package_sha256") != sha256(package)
        or receipt.get("package_size_bytes") != package.stat().st_size
        or receipt.get("signature_verified") is not True
        or receipt.get("cases_passed") != 3
    ):
        raise ValueError(f"signed module receipt does not bind its package: {package.name}")
    return receipt_path, receipt


def staged_packages(directory, architecture, app_version):
    directory = directory.resolve(strict=True)
    if not directory.is_dir() or directory.is_symlink():
        raise ValueError("module artifact staging directory is unsafe")
    result = {}
    for path in directory.glob("CopyPasteModule-*-linux-*.cpmodule"):
        if not path.is_file() or path.is_symlink():
            raise ValueError(f"staged module package is unsafe: {path.name}")
        verified = catalog.read_package(path)
        manifest = verified.manifest
        module_id = manifest["id"]
        if manifest["target"] != {"platform": "linux", "architecture": architecture}:
            raise ValueError(f"staged module target differs from the installed product: {path.name}")
        if module_id not in MODULES:
            raise ValueError(f"unexpected staged Linux module: {module_id}")
        if module_id in result:
            raise ValueError(f"multiple staged packages exist for {module_id}")
        receipt_path, receipt = receipt_for(path, module_id, architecture, app_version)
        result[module_id] = {
            "package": path.resolve(), "manifest": manifest,
            "receipt": receipt, "receipt_path": receipt_path.resolve(),
        }
    if set(result) != set(MODULES):
        missing = sorted(set(MODULES) - set(result))
        raise ValueError(f"signed staged Linux modules are incomplete: {', '.join(missing)}")
    return result


def fixture_manifest(path):
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError(f"module fixture manifest is not JSON: {path.name}") from error
    if not isinstance(value, list) or len(value) != 3:
        raise ValueError("OCR qualification requires three source-image fixtures")
    return value


def ocr_fixture(fixtures):
    root = fixtures / "ocr"
    manifest_path = regular_child(root, "fixtures.json")
    entries = fixture_manifest(manifest_path)
    hashes = {"ocr/fixtures.json": sha256(manifest_path)}
    selected = None
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != {"file", "expected"}:
            raise ValueError("OCR fixture manifest has an invalid entry")
        if not isinstance(entry["expected"], str) or not entry["expected"]:
            raise ValueError("OCR fixture has no expected result")
        image = regular_child(root, entry["file"])
        hashes[f"ocr/{entry['file']}"] = sha256(image)
        if selected is None:
            selected = (image, entry["expected"])
    if selected is None:
        raise ValueError("OCR qualification has no image fixture")
    return selected[0], selected[1], hashes


def semantic_resource_directory(model):
    inventory = sorted(
        [(file["path"], file["sha256"], file["size_bytes"]) for file in model["files"]],
        key=lambda item: item[0],
    )
    encoded = json.dumps(inventory, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    return f"{model['id']}-{hashlib.sha256(encoded).hexdigest()[:16]}"


def semantic_fixture(fixtures, package):
    root = fixtures / "semantic-search"
    fixture_manifest_path = regular_child(root, "search-models.json")
    with zipfile.ZipFile(package) as archive:
        try:
            signed_manifest = archive.read("assets/search-models.json")
        except KeyError as error:
            raise ValueError("signed semantic package has no model manifest") from error
    if fixture_manifest_path.read_bytes() != signed_manifest:
        raise ValueError("semantic fixture manifest differs from the signed package")
    try:
        value = json.loads(signed_manifest)
    except json.JSONDecodeError as error:
        raise ValueError("signed semantic model manifest is not JSON") from error
    models = value.get("models") if isinstance(value, dict) else None
    if not isinstance(models, list) or not models:
        raise ValueError("signed semantic model manifest has no models")
    hashes = {"semantic-search/search-models.json": sha256(fixture_manifest_path)}
    english = None
    for model in models:
        if not isinstance(model, dict) or not isinstance(model.get("id"), str) or not isinstance(model.get("files"), list):
            raise ValueError("signed semantic model manifest is invalid")
        if model.get("languages") == ["en"]:
            if english is not None:
                raise ValueError("signed semantic model manifest has ambiguous English fixtures")
            english = model
    if english is None:
        raise ValueError("signed semantic model manifest has no English fixture")
    for file in english["files"]:
        if not isinstance(file, dict) or not isinstance(file.get("path"), str) or not isinstance(file.get("sha256"), str):
            raise ValueError("signed semantic model file is invalid")
        path = regular_child(root, f"{english['id']}/{file['path']}")
        digest = sha256(path)
        if digest != file["sha256"] or path.stat().st_size != file.get("size_bytes"):
            raise ValueError(f"semantic fixture checksum differs: {english['id']}/{file['path']}")
        hashes[f"semantic-search/{english['id']}/{file['path']}"] = digest
    return english, hashes


def seed_semantic_model(fixtures, application_data, model):
    root = fixtures / "semantic-search" / model["id"]
    destination = (
        application_data / "modules" / "data" / "copypaste.semantic-search"
        / "models" / semantic_resource_directory(model)
    )
    if destination.exists() or destination.is_symlink():
        raise RuntimeError("semantic qualification model destination is not clean")
    destination.mkdir(parents=True, mode=0o700)
    for file in model["files"]:
        source = regular_child(root, file["path"])
        target = destination / file["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        os.chmod(target, 0o600)


def rpc(socket_path, operation):
    request = {"id": 992, "protocol_version": 5, "method": "modules", "params": {"operation": operation}}
    request_bytes = json.dumps(request, separators=(",", ":")).encode() + b"\n"
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as channel:
        channel.settimeout(30)
        channel.connect(str(socket_path))
        channel.sendall(request_bytes)
        response_bytes = channel.makefile("rb").readline()
    try:
        value = json.loads(response_bytes)
    except json.JSONDecodeError as error:
        raise RuntimeError("GUI-owned daemon returned invalid module IPC JSON") from error
    if not isinstance(value, dict) or value.get("id") != 992:
        raise RuntimeError("GUI-owned daemon returned an invalid module IPC response")
    return value, {
        "endpoint_category": "gui_owned_unix_socket",
        "method": "modules",
        "operation": operation["operation"],
        "module_id": operation.get("id"),
        "enabled": operation.get("enabled"),
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "response_sha256": hashlib.sha256(response_bytes).hexdigest(),
    }


def trace(emit, assertion, record, module_id=None):
    record = {key: value for key, value in record.items() if value is not None}
    if module_id is not None:
        record["module_id"] = module_id
    record["success"] = True
    record["assertions"] = [assertion]
    emit(record)


def result(socket_path, operation, emit, assertion, module_id=None):
    response, record = rpc(socket_path, operation)
    if response.get("ok") is not True:
        error = response.get("error")
        raise RuntimeError(f"GUI-owned daemon rejected module operation {operation['operation']}: {error}")
    modules = response.get("data", {}).get("modules")
    encoded = modules.get("json") if isinstance(modules, dict) else None
    if not isinstance(encoded, str):
        raise RuntimeError("GUI-owned daemon did not return a typed module operation result")
    try:
        value = json.loads(encoded)
    except json.JSONDecodeError as error:
        raise RuntimeError("GUI-owned daemon returned invalid module operation JSON") from error
    trace(emit, assertion, record, module_id)
    return value


def inventory(socket_path, emit):
    value = result(socket_path, {"operation": "list"}, emit, "modules")
    if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
        raise RuntimeError("GUI-owned daemon returned an invalid module inventory")
    return value


def installed(inventory_value, module_id):
    matches = [item for item in inventory_value if item.get("id") == module_id]
    if len(matches) != 1:
        raise RuntimeError(f"module inventory does not contain exactly one {module_id}")
    return matches[0]


def invoke(socket_path, module_id, command, arguments, emit):
    return result(socket_path, {
        "operation": "invoke", "id": module_id, "command": command,
        "arguments_json": json.dumps(arguments, sort_keys=True, separators=(",", ":")),
    }, emit, "modules")


def verify_invocation(module_id, output, ocr_image=None, ocr_expected=None):
    if not isinstance(output, dict):
        raise RuntimeError(f"{module_id} returned an invalid native result")
    if module_id == "copypaste.ocr":
        if output.get("kind") != "text" or not isinstance(output.get("text"), str) or ocr_expected not in output["text"]:
            raise RuntimeError("OCR module did not return the staged image text")
    elif module_id == "copypaste.semantic-search":
        vectors = output.get("vectors") if output.get("kind") == "embeddings" else None
        if (not isinstance(vectors, list) or len(vectors) != 1 or not isinstance(vectors[0], list)
                or len(vectors[0]) != 384 or any(not isinstance(value, (int, float)) or not math.isfinite(value) for value in vectors[0])):
            raise RuntimeError("semantic module did not return one finite 384-dimensional embedding")
    elif module_id == "copypaste.supabase":
        status = output.get("data", {}).get("status") if output.get("kind") == "data" else None
        if not isinstance(status, dict) or status.get("configured") is not False or status.get("signed_in") is not False:
            raise RuntimeError("Supabase module did not return a signed-out local account state")


def module_arguments(module_id, ocr_image):
    if module_id == "copypaste.ocr":
        return "recognize-image", {"image_path": str(ocr_image)}
    if module_id == "copypaste.semantic-search":
        return "embed-text", {"role": "query", "text": "paying for housing"}
    if module_id == "copypaste.supabase":
        return "status", {}
    raise AssertionError(module_id)


def qualify_modules(socket_path, artifacts, fixtures, application_data, architecture, app_version, evidence_dir, restart, emit):
    """Exercise signed modules through a running GUI-owned daemon.

    ``restart`` must replace the installed GUI process and return its new IPC
    socket.  The caller deliberately owns process lifetime so this helper can
    never turn an inventory or a synthetic receipt into lifecycle evidence.
    """
    artifacts = artifacts.resolve(strict=True)
    fixtures = fixtures.resolve(strict=True)
    application_data = application_data.resolve(strict=True)
    evidence_dir = evidence_dir.resolve()
    packages = staged_packages(artifacts, architecture, app_version)
    ocr_image, ocr_expected, fixture_hashes = ocr_fixture(fixtures)
    semantic_model, semantic_hashes = semantic_fixture(fixtures, packages["copypaste.semantic-search"]["package"])
    fixture_hashes.update(semantic_hashes)
    evidence = []
    for module_id in MODULES:
        package = packages[module_id]
        install = result(socket_path, {
            "operation": "install", "package_path": str(package["package"]),
        }, emit, "modules", module_id)
        if not isinstance(install, dict) or install.get("id") != module_id:
            raise RuntimeError(f"GUI-owned daemon did not install {module_id}")
        installed(inventory(socket_path, emit), module_id)
        if module_id == "copypaste.semantic-search":
            seed_semantic_model(fixtures, application_data, semantic_model)
            result(socket_path, {
                "operation": "set_preferences", "id": module_id,
                "values_json": json.dumps({"languages": ["en"]}, separators=(",", ":")),
            }, emit, "modules")
        result(socket_path, {"operation": "set_enabled", "id": module_id, "enabled": True}, emit, "modules")
        if installed(inventory(socket_path, emit), module_id).get("enabled") is not True:
            raise RuntimeError(f"GUI-owned daemon did not enable {module_id}")
        command, arguments = module_arguments(module_id, ocr_image)
        output = invoke(socket_path, module_id, command, arguments, emit)
        verify_invocation(module_id, output, ocr_image, ocr_expected)
        result(socket_path, {"operation": "set_enabled", "id": module_id, "enabled": False}, emit, "modules")
        if installed(inventory(socket_path, emit), module_id).get("enabled") is not False:
            raise RuntimeError(f"GUI-owned daemon did not disable {module_id}")
        result(socket_path, {"operation": "remove", "id": module_id}, emit, "modules")
        socket_path = restart()
        if any(item.get("id") == module_id for item in inventory(socket_path, emit)):
            raise RuntimeError(f"GUI-owned daemon did not finish removing {module_id} after restart")
        evidence.append({
            "id": module_id, "package": package["package"].name,
            "package_sha256": sha256(package["package"]), "package_size_bytes": package["package"].stat().st_size,
            "native_receipt": package["receipt_path"].name,
            "native_receipt_sha256": sha256(package["receipt_path"]),
            "fixture_sha256": fixture_hashes if module_id != "copypaste.supabase" else {},
        })
    evidence_dir.mkdir(parents=True, exist_ok=True)
    output = evidence_dir / "linux-module-qualification.json"
    output.write_text(json.dumps({
        "schema": 1, "target": {"platform": "linux", "architecture": architecture},
        "modules": evidence,
    }, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return socket_path
