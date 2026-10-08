import 'package:pub_semver/pub_semver.dart';

enum ModulesSection { marketplace, installed }

enum ModuleAvailability { available, appVersion, systemVersion, platform }

class ModuleMarketplaceTarget {
  const ModuleMarketplaceTarget({
    required this.platform,
    required this.architecture,
    required this.appVersion,
    this.systemVersion,
  });

  final String platform;
  final String architecture;
  final Version? appVersion;
  final Version? systemVersion;
}

class ModuleArtifact {
  const ModuleArtifact({
    required this.downloadUri,
    required this.sizeBytes,
    required this.sha256,
    this.minimumSystemVersion,
  });

  final Uri downloadUri;
  final int sizeBytes;
  final String sha256;
  final Version? minimumSystemVersion;
}

class MarketplaceModule {
  const MarketplaceModule({
    required this.id,
    required this.title,
    required this.description,
    required this.version,
    required this.artifact,
    this.appVersions,
    this.availability = ModuleAvailability.available,
    this.unavailableReason,
    this.systemRequirement,
  });

  final String id;
  final String title;
  final String description;
  final Version version;
  final ModuleArtifact? artifact;
  final VersionConstraint? appVersions;
  final ModuleAvailability availability;
  final String? unavailableReason;
  final String? systemRequirement;

  /// Requirements already explain version incompatibility on the card.
  String? get availabilityNotice {
    if (availability == ModuleAvailability.systemVersion &&
        systemRequirement != null) {
      return null;
    }
    if (availability == ModuleAvailability.appVersion &&
        appRequirement != null &&
        (unavailableReason?.startsWith(
              'This module does not support CopyPaste ',
            ) ??
            false)) {
      return null;
    }
    return unavailableReason;
  }

  bool get canInstall =>
      artifact != null && availability == ModuleAvailability.available;

  String? get downloadSize =>
      artifact == null ? null : formatModuleSize(artifact!.sizeBytes);
  String? get appRequirement {
    final constraint = appVersions;
    if (constraint == null || constraint.isAny) return null;
    if (constraint is VersionRange) {
      final min = constraint.min;
      final max = constraint.max;
      if (min != null && max != null) {
        return 'CopyPaste ${constraint.includeMin ? '≥' : '>'}$min, ${constraint.includeMax ? '≤' : '<'}${_displayUpperVersion(max)}';
      }
      if (min != null) {
        return 'CopyPaste ${constraint.includeMin ? '≥' : '>'}$min';
      }
      if (max != null) {
        return 'CopyPaste ${constraint.includeMax ? '≤' : '<'}${_displayUpperVersion(max)}';
      }
    }
    return 'CopyPaste $constraint';
  }

  // pub_semver represents an exclusive stable upper bound with a -0 sentinel.
  String _displayUpperVersion(Version version) =>
      version.preRelease.length == 1 && version.preRelease.single == 0
      ? '${version.major}.${version.minor}.${version.patch}'
      : version.toString();
}

String formatModuleSize(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}
