import 'dart:io';

import 'package:pub_semver/pub_semver.dart';

/// Successful installation proves these cached packages are obsolete. Pending
/// newer versions remain available for permission dialogs, retry and recovery.
Future<void> removeInstalledUpdatePackages(
  Directory directory,
  Version installed,
) async {
  final package = RegExp(
    r'^CopyPaste-v(.+?)-(?:android(?:-(?:arm64|armv7))?\.apk|windows-x86_64-setup\.exe|linux-(?:x86_64|aarch64)\.(?:deb|rpm|AppImage))(?:\.sig|\.part)?$',
  );
  try {
    if (!await directory.exists()) return;
    await for (final entry in directory.list(followLinks: false)) {
      if (entry is! File) continue;
      final match = package.firstMatch(
        entry.path.split(Platform.pathSeparator).last,
      );
      Version? version;
      if (match != null) {
        try {
          version = Version.parse(match.group(1)!);
        } on FormatException {
          continue;
        }
      }
      if (version == null || version > installed) continue;
      try {
        await entry.delete();
      } on FileSystemException {
        // Windows may still be finishing an installer. Retry next check.
      }
    }
  } on FileSystemException {
    // Cache maintenance must not prevent offline update recovery.
  }
}
