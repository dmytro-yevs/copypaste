#!/usr/bin/env python3
"""Stage authenticated Linux module packages and offline qualification fixtures."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
from urllib.request import urlopen


ROOT = Path(__file__).resolve().parents[2]
CATALOG_SPEC = importlib.util.spec_from_file_location(
    "copypaste_module_catalog", ROOT / "scripts/modules/catalog.py",
)
catalog = importlib.util.module_from_spec(CATALOG_SPEC)
assert CATALOG_SPEC.loader is not None
CATALOG_SPEC.loader.exec_module(catalog)

MODULES = {
    "supabase": "copypaste.supabase",
    "semantic-search": "copypaste.semantic-search",
    "ocr": "copypaste.ocr",
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def regular(path: Path, label: str) -> Path:
    try:
        resolved = path.resolve(strict=True)
    except FileNotFoundError as error:
        raise ValueError(f"{label} is missing: {path}") from error
    if path.is_symlink() or not resolved.is_file():
        raise ValueError(f"{label} is unsafe: {path}")
    return resolved


def copy_regular(source: Path, destination: Path, label: str) -> None:
    source = regular(source, label)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)


def package_filename(module_id: str, version: str, architecture: str) -> str:
    return f"CopyPasteModule-{module_id}-v{version}-linux-{architecture}.cpmodule"


def source_module(source_root: Path, name: str, app_version: str) -> dict:
    path = regular(source_root / "modules" / name / "module.json", "module source manifest")
    value = json.loads(path.read_text(encoding="utf-8"))
    if value.get("id") != MODULES[name] or value.get("app_versions") != f">={app_version}, <2.0.0":
        raise ValueError(f"module source manifest does not bind {name} to application {app_version}")
    version = value.get("version")
    if not isinstance(version, str) or not version:
        raise ValueError(f"module source manifest has no version: {name}")
    return value


def verify_and_stage_package(downloads: Path, staged: Path, name: str, architecture: str,
                             app_version: str, commit: str, run_id: str, source_root: Path) -> None:
    module_id = MODULES[name]
    source = source_module(source_root, name, app_version)
    package = regular(downloads / name / package_filename(module_id, source["version"], architecture), "module package")
    receipt_path = regular(package.with_name(package.name + ".receipt.json"), "module receipt")
    verified = catalog.read_package(package)
    manifest = verified.manifest
    if (
        manifest.get("id") != module_id
        or manifest.get("version") != source["version"]
        or manifest.get("app_versions") != source["app_versions"]
        or manifest.get("target") != {"platform": "linux", "architecture": architecture}
    ):
        raise ValueError(f"signed package metadata differs from source provenance: {package.name}")
    try:
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError(f"module receipt is not JSON: {receipt_path.name}") from error
    if (
        receipt.get("commit") != commit
        or receipt.get("run_id") != run_id
        or receipt.get("module_id") != module_id
        or receipt.get("module_version") != source["version"]
        or receipt.get("app_version") != app_version
        or receipt.get("target") != {"platform": "linux", "architecture": architecture}
        or receipt.get("package_sha256") != sha256(package)
        or receipt.get("package_size_bytes") != package.stat().st_size
        or receipt.get("signature_verified") is not True
        or receipt.get("cases_passed") != 3
        or receipt.get("removal_completed_after_restart") is not True
        or receipt.get("restart_required") != (name == "ocr")
    ):
        raise ValueError(f"module receipt does not bind the exact signed package: {package.name}")
    copy_regular(package, staged / package.name, "module package")
    copy_regular(receipt_path, staged / receipt_path.name, "module receipt")


def stage_ocr_fixtures(source_root: Path, fixtures: Path) -> None:
    original = source_root / "scripts/modules/fixtures"
    manifest = json.loads(regular(original / "fixtures.json", "OCR fixture manifest").read_text(encoding="utf-8"))
    if not isinstance(manifest, list) or len(manifest) != 3:
        raise ValueError("OCR qualification requires exactly three source-image fixtures")
    copy_regular(original / "fixtures.json", fixtures / "ocr/fixtures.json", "OCR fixture manifest")
    for item in manifest:
        if not isinstance(item, dict) or set(item) != {"file", "expected"} or not isinstance(item["file"], str):
            raise ValueError("OCR fixture manifest is invalid")
        if Path(item["file"]).name != item["file"]:
            raise ValueError("OCR fixture filename is invalid")
        copy_regular(original / item["file"], fixtures / "ocr" / item["file"], "OCR source image")


def fetch_semantic_fixture(source_root: Path, fixtures: Path) -> None:
    source = regular(source_root / "modules/semantic-search/assets/search-models.json", "semantic model manifest")
    document = json.loads(source.read_text(encoding="utf-8"))
    models = document.get("models") if isinstance(document, dict) else None
    english = next((model for model in models or [] if model.get("id") == "minilm-en-v1" and model.get("languages") == ["en"]), None)
    if not isinstance(english, dict) or not isinstance(english.get("files"), list) or not english["files"]:
        raise ValueError("semantic model manifest lacks the pinned English profile")
    destination = fixtures / "semantic-search"
    copy_regular(source, destination / "search-models.json", "semantic model manifest")
    for item in english["files"]:
        if set(item) != {"path", "url", "sha256", "size_bytes"} or not isinstance(item["path"], str):
            raise ValueError("semantic model file manifest is invalid")
        relative = Path(item["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError("semantic model path is invalid")
        target = destination / english["id"] / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(dir=target.parent, delete=False) as temporary:
            temporary_path = Path(temporary.name)
            try:
                with urlopen(item["url"], timeout=120) as response:
                    shutil.copyfileobj(response, temporary, 1024 * 1024)
                temporary.flush()
                if temporary_path.stat().st_size != item["size_bytes"] or sha256(temporary_path) != item["sha256"]:
                    raise ValueError(f"semantic fixture checksum differs: {english['id']}/{item['path']}")
                temporary_path.replace(target)
            finally:
                temporary_path.unlink(missing_ok=True)


def stage(args: argparse.Namespace) -> None:
    source_root = args.source_root.resolve(strict=True)
    downloads = args.downloads.resolve(strict=True)
    if source_root.is_symlink() or downloads.is_symlink() or not downloads.is_dir():
        raise ValueError("module qualification inputs must be real directories")
    modules = args.module_artifacts.resolve()
    fixtures = args.module_fixtures.resolve()
    if modules.exists() or fixtures.exists():
        raise ValueError("module qualification staging directories must be clean")
    modules.mkdir(mode=0o700)
    fixtures.mkdir(mode=0o700)
    try:
        for name in MODULES:
            verify_and_stage_package(downloads, modules, name, args.architecture, args.app_version,
                                     args.commit, getattr(args, name.replace("-", "_") + "_run_id"), source_root)
        stage_ocr_fixtures(source_root, fixtures)
        fetch_semantic_fixture(source_root, fixtures)
    except Exception:
        shutil.rmtree(modules)
        shutil.rmtree(fixtures)
        raise


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, default=ROOT)
    parser.add_argument("--downloads", required=True, type=Path)
    parser.add_argument("--module-artifacts", required=True, type=Path)
    parser.add_argument("--module-fixtures", required=True, type=Path)
    parser.add_argument("--architecture", required=True, choices=("x86_64", "aarch64"))
    parser.add_argument("--app-version", required=True)
    parser.add_argument("--commit", required=True)
    for name in MODULES:
        parser.add_argument("--" + name + "-run-id", dest=name.replace("-", "_") + "_run_id", required=True)
    args = parser.parse_args()
    stage(args)
    print("staged authenticated Linux modules and offline qualification fixtures")


if __name__ == "__main__":
    main()
