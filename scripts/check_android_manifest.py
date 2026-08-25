#!/usr/bin/env python3
import argparse
from pathlib import Path
import sys
import xml.etree.ElementTree as ET


ANDROID = "{http://schemas.android.com/apk/res/android}"
QUERY_ALL_PACKAGES = "android.permission.QUERY_ALL_PACKAGES"
REQUIRED_PERMISSIONS = frozenset(
    {
        "android.permission.CAMERA",
        "android.permission.CHANGE_WIFI_MULTICAST_STATE",
        "android.permission.FOREGROUND_SERVICE",
        "android.permission.FOREGROUND_SERVICE_SPECIAL_USE",
        "android.permission.INTERNET",
        "android.permission.POST_NOTIFICATIONS",
        "android.permission.READ_LOGS",
        "android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS",
        "android.permission.REQUEST_INSTALL_PACKAGES",
        "android.permission.SYSTEM_ALERT_WINDOW",
        "moe.shizuku.manager.permission.API_V23",
    }
)


class ManifestError(Exception):
    pass


def android_attribute(element, name):
    return element.get(ANDROID + name)


def named_children(parent, tag, name):
    return [
        child
        for child in parent.findall(tag)
        if android_attribute(child, "name") == name
    ]


def require_one(parent, tag, name, errors):
    matches = named_children(parent, tag, name)
    if len(matches) != 1:
        errors.append(f"expected one <{tag}> named {name}; found {len(matches)}")
        return None
    return matches[0]


def intent_has(intent, tag, name):
    return any(
        android_attribute(child, "name") == name for child in intent.findall(tag)
    )


def require_attributes(element, expected, label, errors):
    for name, value in expected.items():
        if android_attribute(element, name) != value:
            errors.append(f"{label} must set android:{name}={value}")


def manifest_errors(source, query_all_packages_exempt=False):
    try:
        root = ET.fromstring(source)
    except ET.ParseError as error:
        return [f"AndroidManifest.xml is not valid XML: {error}"]
    if root.tag != "manifest":
        return ["AndroidManifest.xml root must be <manifest>"]

    errors = []
    permissions = {
        android_attribute(element, "name")
        for element in root.findall("uses-permission")
    }
    missing_permissions = sorted(REQUIRED_PERMISSIONS - permissions)
    for permission in missing_permissions:
        errors.append(f"missing required permission {permission}")
    if QUERY_ALL_PACKAGES in permissions and not query_all_packages_exempt:
        errors.append("QUERY_ALL_PACKAGES requires an ADR exemption")

    query_blocks = root.findall("queries")
    if len(query_blocks) != 1:
        errors.append(f"expected one <queries> block; found {len(query_blocks)}")
    else:
        intents = query_blocks[0].findall("intent")
        required_queries = (
            ("android.intent.action.MAIN", "android.intent.category.LAUNCHER"),
            ("android.intent.action.MAIN", "android.intent.category.LEANBACK_LAUNCHER"),
        )
        for action, category in required_queries:
            if not any(
                intent_has(intent, "action", action)
                and intent_has(intent, "category", category)
                for intent in intents
            ):
                errors.append(f"missing intent query for {action} with {category}")
        require_one(
            query_blocks[0],
            "package",
            "moe.shizuku.privileged.api",
            errors,
        )

    applications = root.findall("application")
    if len(applications) != 1:
        errors.append(f"expected one <application>; found {len(applications)}")
        return errors
    application = applications[0]

    tile = require_one(application, "service", ".CaptureTileService", errors)
    if tile is not None:
        require_attributes(
            tile,
            {
                "exported": "true",
                "permission": "android.permission.BIND_QUICK_SETTINGS_TILE",
            },
            "CaptureTileService",
            errors,
        )
        if not any(
            intent_has(intent, "action", "android.service.quicksettings.action.QS_TILE")
            for intent in tile.findall("intent-filter")
        ):
            errors.append("CaptureTileService is missing its QS_TILE intent filter")

    capture = require_one(application, "service", ".CaptureService", errors)
    if capture is not None:
        require_attributes(
            capture,
            {"exported": "false", "foregroundServiceType": "specialUse"},
            "CaptureService",
            errors,
        )
        subtype = require_one(
            capture,
            "property",
            "android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE",
            errors,
        )
        if subtype is not None and not android_attribute(subtype, "value"):
            errors.append("CaptureService special-use subtype must explain its purpose")

    provider = require_one(
        application, "provider", "rikka.shizuku.ShizukuProvider", errors
    )
    if provider is not None:
        require_attributes(
            provider,
            {
                "enabled": "true",
                "exported": "true",
                "permission": "android.permission.INTERACT_ACROSS_USERS_FULL",
            },
            "ShizukuProvider",
            errors,
        )
    return errors


def has_query_all_packages_adr(directory):
    try:
        return any(
            path.is_file() and "query-all-packages" in path.name.lower()
            for path in directory.iterdir()
        )
    except OSError:
        return False


def validate_manifest_file(manifest, adr_directory):
    try:
        source = manifest.read_text(encoding="utf-8")
    except OSError as error:
        raise ManifestError("AndroidManifest.xml is missing or unreadable") from error
    errors = manifest_errors(source, has_query_all_packages_adr(adr_directory))
    if errors:
        raise ManifestError("\n".join(errors))


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=Path)
    parser.add_argument("adr_directory", type=Path)
    args = parser.parse_args(argv)
    try:
        validate_manifest_file(args.manifest, args.adr_directory)
    except ManifestError as error:
        for message in str(error).splitlines():
            print(f"FAIL: {message}", file=sys.stderr)
        return 1
    print("PASS: Android manifest uses structured narrow package visibility")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
