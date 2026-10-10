"""Pure validation for private compositor closure license provenance."""
from __future__ import annotations

from pathlib import PurePosixPath
from typing import Any

SHA256_LENGTH = 64
CANONICAL_STANDARD_LICENSES = {
    "https://www.gnu.org/licenses/lgpl-3.0.txt": "e3a994d82e644b03a792a930f574002658412f62407f5fee083f2555c5f23118",
    "https://www.gnu.org/licenses/gpl-3.0.txt": "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986",
}

BASE_KEYS = frozenset({
    "package", "license_package", "license_evr", "license_source_rpm",
    "license", "path", "sha256", "license_origin",
})
SOURCE_KEYS = BASE_KEYS | {
    "license_archive", "license_archive_sha256", "license_archive_supplier",
    "license_archive_evr", "license_source_member",
}
STANDARD_KEYS = BASE_KEYS | {"standard_license_url", "standard_license_sha256"}


def safe_source_member(value: Any) -> bool:
    if not isinstance(value, str) or not value or len(value) > 512 or "\x00" in value:
        return False
    path = PurePosixPath(value)
    return (str(path) == value and not path.is_absolute()
            and all(part not in {"", ".", ".."} for part in path.parts))


def validate_origin(record: Any) -> str | None:
    """Return a compact error rather than importing packaging-specific errors."""
    if not isinstance(record, dict):
        return "record"
    origin = record.get("license_origin")
    if origin == "installed-rpm":
        if set(record) != BASE_KEYS:
            return "installed-schema"
        return None
    if origin == "source-rpm":
        if set(record) != SOURCE_KEYS:
            return "source-schema"
        if record.get("license_archive") != record.get("license_source_rpm"):
            return "source-archive"
        if not safe_source_member(record.get("license_source_member")):
            return "source-member"
        return None
    if origin == "standard-license":
        if set(record) != STANDARD_KEYS:
            return "standard-schema"
        url = record.get("standard_license_url")
        expected = CANONICAL_STANDARD_LICENSES.get(url)
        if expected is None:
            return "standard-url"
        if record.get("standard_license_sha256") != expected:
            return "standard-expected-sha256"
        if record.get("sha256") != expected:
            return "standard-bytes-sha256"
        return None
    return "origin"
