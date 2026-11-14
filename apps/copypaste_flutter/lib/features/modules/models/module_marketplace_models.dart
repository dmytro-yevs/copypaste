import '../../../shared/format_bytes.dart';
import '../../../l10n/l10n.dart';

import 'package:pub_semver/pub_semver.dart';

import 'module_models.dart';

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
    required String title,
    required String description,
    this.localizations = const {},
    required this.version,
    required this.artifact,
    this.supportedPlatforms = const [],
    this.appVersions,
    this.availability = ModuleAvailability.available,
    this.unavailableReason,
    this.systemRequirement,
  }) : _title = title,
       _description = description;

  final String id;
  final String _title;
  final String _description;

  /// `locale -> {title, description}` translations from the catalog.
  final Map<String, Map<String, String>> localizations;

  String get title => _localized('title') ?? _title;
  String get description => _localized('description') ?? _description;

  /// The exact interface language tag first, then each shorter prefix, then
  /// the English catalog strings.
  String? _localized(String key) {
    if (localizations.isEmpty) return null;
    var tag = AppStrings.locale.toLanguageTag().toLowerCase();
    while (true) {
      for (final MapEntry(key: locale, value: strings)
          in localizations.entries) {
        final value = locale.toLowerCase() == tag ? strings[key] : null;
        if (value != null) return value;
      }
      final end = tag.lastIndexOf('-');
      if (end < 0) return null;
      tag = tag.substring(0, end);
    }
  }

  final Version version;
  final ModuleArtifact? artifact;
  final List<ModulePlatform> supportedPlatforms;
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
        unavailableReason != AppStrings.current.modulesAppVersionUnverified) {
      return null;
    }
    return unavailableReason;
  }

  bool get canInstall =>
      artifact != null && availability == ModuleAvailability.available;

  String? get downloadSize =>
      artifact == null ? null : formatBytes(artifact!.sizeBytes);
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
