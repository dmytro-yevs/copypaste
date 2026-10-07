import 'package:pub_semver/pub_semver.dart';

enum ModulesSection { marketplace, installed }

class ModuleMarketplaceTarget {
  const ModuleMarketplaceTarget({
    required this.platform,
    required this.architecture,
    required this.appVersion,
  });

  final String platform;
  final String architecture;
  final Version appVersion;
}

class ModuleArtifact {
  const ModuleArtifact({
    required this.downloadUri,
    required this.sizeBytes,
    required this.sha256,
  });

  final Uri downloadUri;
  final int sizeBytes;
  final String sha256;
}

class MarketplaceModule {
  const MarketplaceModule({
    required this.id,
    required this.title,
    required this.description,
    required this.version,
    required this.artifact,
  });

  final String id;
  final String title;
  final String description;
  final Version version;
  final ModuleArtifact artifact;

  String get downloadSize => formatModuleSize(artifact.sizeBytes);
}

String formatModuleSize(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}
