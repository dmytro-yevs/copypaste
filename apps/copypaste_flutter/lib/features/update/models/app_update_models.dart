import 'package:pub_semver/pub_semver.dart';

enum AppUpdateTarget { macos, windows, android }

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
  const AppUpdateAvailability.available() : available = true, reason = null;

  const AppUpdateAvailability.unavailable(this.reason) : available = false;

  final bool available;
  final String? reason;
}

enum AppUpdateInstallResult { started, permissionRequired, restartRequired }

class AppUpdateException implements Exception {
  const AppUpdateException(this.message);

  final String message;

  @override
  String toString() => message;
}
