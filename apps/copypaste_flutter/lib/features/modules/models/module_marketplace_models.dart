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
        return 'CopyPaste ${constraint.includeMin ? '' : 'after '}$min to ${constraint.includeMax ? '' : 'before '}$max';
      }
      if (min != null) {
        return 'CopyPaste $min ${constraint.includeMin ? 'or newer' : 'or later'}';
      }
      if (max != null) {
        return 'CopyPaste ${constraint.includeMax ? 'up to' : 'before'} $max';
      }
    }
    return 'CopyPaste $constraint';
  }
}

String formatModuleSize(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}
