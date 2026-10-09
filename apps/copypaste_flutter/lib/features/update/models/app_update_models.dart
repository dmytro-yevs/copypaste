import 'package:pub_semver/pub_semver.dart';

enum AppUpdateTarget { macos, windows, android, linux }

enum AndroidAppUpdateArchitecture { arm64, armv7 }

enum LinuxAppUpdateArchitecture { x86_64, aarch64 }

enum LinuxAppUpdatePackage { appImage, deb, rpm }

/// The native runner identifies the installed package before an update is
/// selected, so the updater never substitutes another Linux package format.
class LinuxAppUpdateInstallation {
  const LinuxAppUpdateInstallation({
    required this.package,
    required this.architecture,
  });

  final LinuxAppUpdatePackage package;
  final LinuxAppUpdateArchitecture architecture;

  static LinuxAppUpdateInstallation? parse({
    required String? installationType,
    required String? architecture,
  }) {
    final package = switch (installationType) {
      'appimage' => LinuxAppUpdatePackage.appImage,
      'deb' => LinuxAppUpdatePackage.deb,
      'rpm' => LinuxAppUpdatePackage.rpm,
      _ => null,
    };
    final parsedArchitecture = switch (architecture) {
      'x86_64' => LinuxAppUpdateArchitecture.x86_64,
      'aarch64' => LinuxAppUpdateArchitecture.aarch64,
      _ => null,
    };
    if (package == null || parsedArchitecture == null) return null;
    return LinuxAppUpdateInstallation(
      package: package,
      architecture: parsedArchitecture,
    );
  }
}

enum AppUpdatePhase {
  idle,
  checking,
  upToDate,
  available,
  downloading,
  installing,
  permissionRequired,
  restartRequired,
  unavailable,
  error,
}

class AppReleaseAsset {
  const AppReleaseAsset({
    required this.name,
    required this.downloadUri,
    required this.sha256,
    required this.sizeBytes,
    required this.signatureUri,
    required this.signatureSha256,
    required this.signatureSizeBytes,
  });

  final String name;
  final Uri downloadUri;
  final String sha256;
  final int sizeBytes;
  final Uri signatureUri;
  final String signatureSha256;
  final int signatureSizeBytes;
}

class AppRelease {
  const AppRelease({
    required this.version,
    required this.releaseUri,
    required this.prerelease,
    this.publishedAt,
    this.asset,
  });

  final Version version;
  final Uri releaseUri;
  final bool prerelease;
  final DateTime? publishedAt;
  final AppReleaseAsset? asset;
}

class DownloadedAppUpdate {
  const DownloadedAppUpdate({required this.path, required this.asset});

  final String path;
  final AppReleaseAsset asset;
}

class AppUpdateAvailability {
  const AppUpdateAvailability.available({this.linuxInstallation})
    : available = true,
      reason = null;

  const AppUpdateAvailability.unavailable(this.reason)
    : available = false,
      linuxInstallation = null;

  final bool available;
  final String? reason;
  final LinuxAppUpdateInstallation? linuxInstallation;
}

enum AppUpdateInstallResult {
  started,
  permissionRequired,
  restartRequired,
  installed,
}

class AppUpdateException implements Exception {
  const AppUpdateException(this.message);

  final String message;

  @override
  String toString() => message;
}
