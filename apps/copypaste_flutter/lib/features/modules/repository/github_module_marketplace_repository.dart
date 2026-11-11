import '../../../shared/network/github_artifact_downloader.dart';
import '../../../platform/files/temporary_artifacts.dart';

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:pub_semver/pub_semver.dart';

import '../../../shared/security/minisign_verifier.dart';
import '../../../l10n/l10n.dart';
import '../models/module_marketplace_models.dart';
import '../models/module_models.dart';
import 'module_marketplace_repository.dart';
import 'modules_repository.dart';

const moduleRepositoryPath = githubReleaseAssetPath;
const moduleCatalogName = 'modules-v2.json';
const maximumModulePackageBytes = 2 * 1024 * 1024 * 1024;

/// The catalog authenticates metadata and compressed packages. The runtime
/// independently authenticates the manifest and every extracted file.
class GitHubModuleMarketplaceRepository implements ModuleMarketplaceRepository {
  GitHubModuleMarketplaceRepository({
    required Future<Directory> Function() temporaryDirectory,
    required Future<ModuleMarketplaceTarget> Function() currentTarget,
    HttpClient? client,
    MinisignVerifier? signatureVerifier,
  }) : _temporaryDirectory = temporaryDirectory,
       _currentTarget = currentTarget,
       _client = client ?? HttpClient(),
       _signatureVerifier =
           signatureVerifier ??
           MinisignVerifier(publicKeyBase64: copyPasteUpdaterPublicKey);

  static final _catalogUri = Uri.https(
    'github.com',
    '${moduleRepositoryPath}modules/$moduleCatalogName',
  );
  final Future<Directory> Function() _temporaryDirectory;
  final Future<ModuleMarketplaceTarget> Function() _currentTarget;
  final HttpClient _client;
  late final _downloader = GitHubArtifactDownloader(
    _client,
    userAgent: 'CopyPaste modules',
  );
  final MinisignVerifier _signatureVerifier;

  @override
  Future<List<MarketplaceModule>> list() async {
    final directory = await _createStagingDirectory('module-catalog-');
    final lease = await TemporaryDirectoryLease.acquire(directory);
    try {
      final catalog = File('${directory.path}/$moduleCatalogName');
      if (!await _download(
        _catalogUri,
        catalog,
        maximumBytes: 2 * 1024 * 1024,
        allowMissing: true,
      )) {
        return const [];
      }
      final signature = File('${directory.path}/$moduleCatalogName.sig');
      await _download(
        Uri.parse('$_catalogUri.sig'),
        signature,
        maximumBytes: 64 * 1024,
      );
      if (!await _signatureVerifier.verifyFile(
        file: catalog,
        encodedSignature: await signature.readAsString(),
        expectedFileName: moduleCatalogName,
      )) {
        throw ModulesException(
          AppStrings.current.modulesCatalogSignatureInvalid,
        );
      }
      return const ModuleCatalogParser().parse(
        await catalog.readAsString(),
        await _currentTarget(),
      );
    } on SocketException {
      throw ModulesException(
        AppStrings.current.modulesMarketplaceConnectFailed,
      );
    } on TimeoutException {
      throw ModulesException(AppStrings.current.modulesMarketplaceTimeout);
    } finally {
      await lease.dispose();
    }
  }

  @override
  Future<SelectedModulePackage> download(
    MarketplaceModule module, {
    required void Function(double progress) onProgress,
  }) async {
    final artifact = module.artifact;
    if (artifact == null || !module.canInstall) {
      throw ModulesException(AppStrings.current.modulesIncompatible);
    }
    validateModuleAssetUri(artifact.downloadUri);
    if (artifact.sizeBytes <= 0 ||
        artifact.sizeBytes > maximumModulePackageBytes) {
      throw ModulesException(AppStrings.current.modulesPackageSizeInvalid);
    }
    final directory = await _createStagingDirectory('module-download-');
    final lease = await TemporaryDirectoryLease.acquire(directory);
    try {
      final file = File('${directory.path}/package.cpmodule');
      await _download(
        artifact.downloadUri,
        file,
        maximumBytes: artifact.sizeBytes,
        onProgress: onProgress,
        expectedSize: artifact.sizeBytes,
        expectedSha256: artifact.sha256,
      );
      return SelectedModulePackage(path: file.path, dispose: lease.dispose);
    } catch (error) {
      await lease.dispose();
      if (error is SocketException) {
        throw ModulesException(AppStrings.current.modulesDownloadInterrupted);
      }
      if (error is TimeoutException) {
        throw ModulesException(AppStrings.current.modulesDownloadTimeout);
      }
      rethrow;
    }
  }

  Future<Directory> _createStagingDirectory(String prefix) async {
    final root = await _temporaryDirectory();
    await root.create(recursive: true);
    final staging = Directory('${root.path}/copypaste-staging');
    await staging.create(recursive: true);
    return staging.createTemp(prefix);
  }

  Future<bool> _download(
    Uri uri,
    File destination, {
    required int maximumBytes,
    void Function(double)? onProgress,
    bool allowMissing = false,
    int? expectedSize,
    String? expectedSha256,
  }) async {
    try {
      return await _downloader.download(
        uri,
        destination,
        maximumBytes: maximumBytes,
        expectedSize: expectedSize,
        expectedSha256: expectedSha256,
        onProgress: onProgress,
        allowMissing: allowMissing,
      );
    } on ArtifactDownloadException catch (error) {
      final strings = AppStrings.current;
      throw ModulesException(switch (error.failure) {
        ArtifactDownloadFailure.untrustedUrl =>
          strings.modulesPackageUrlUntrusted,
        ArtifactDownloadFailure.untrustedRedirect =>
          strings.modulesDownloadLeftGithub,
        ArtifactDownloadFailure.redirects =>
          strings.modulesDownloadTooManyRedirects,
        ArtifactDownloadFailure.tooLarge => strings.modulesDownloadTooLarge,
        ArtifactDownloadFailure.integrity =>
          strings.modulesPackageIntegrityFailed,
        ArtifactDownloadFailure.unavailable =>
          error.statusCode == HttpStatus.notFound
              ? strings.modulesDownloadUnavailable
              : strings.modulesMarketplaceUnavailableRetry,
      });
    }
  }

  @override
  void dispose() => _client.close(force: true);
}

void validateModuleAssetUri(Uri uri) {
  try {
    GitHubArtifactDownloader.validateAssetUri(uri);
  } on ArtifactDownloadException {
    throw ModulesException(AppStrings.current.modulesPackageUrlUntrusted);
  }
}

class ModuleCatalogParser {
  const ModuleCatalogParser();

  List<MarketplaceModule> parse(String body, ModuleMarketplaceTarget target) {
    try {
      final data = jsonDecode(body) as Map<String, dynamic>;
      if (data['schema_version'] != 1) throw const FormatException();
      final entries = data['modules'] as List<dynamic>;
      if (entries.length > 1000) throw const FormatException();
      final modules = <MarketplaceModule>[];
      final ids = <String>{};
      for (final raw in entries) {
        final entry = raw as Map<String, dynamic>;
        final id = entry['id'] as String;
        final title = entry['title'] as String;
        final description = entry['description'] as String;
        final version = Version.parse(entry['version'] as String);
        final compatibility = VersionConstraint.parse(
          (entry['app_versions'] as String).replaceAll(',', ' '),
        );
        if (!RegExp(r'^copypaste\.[a-z0-9][a-z0-9.-]*$').hasMatch(id) ||
            !ids.add(id) ||
            title.trim().isEmpty ||
            title.length > 200 ||
            description.trim().isEmpty ||
            description.length > 4000 ||
            version.isPreRelease) {
          throw const FormatException();
        }
        ModuleArtifact? selected;
        final targets = <String>{};
        final artifactPlatforms = <String>{};
        for (final rawArtifact in entry['artifacts'] as List<dynamic>) {
          final artifact = rawArtifact as Map<String, dynamic>;
          final platform = artifact['platform'] as String;
          final architecture = artifact['architecture'] as String;
          final uri = Uri.parse(artifact['url'] as String);
          final size = artifact['size_bytes'] as int;
          final digest = artifact['sha256'] as String;
          final minimumSystemVersion =
              artifact['minimum_system_version'] == null
              ? null
              : Version.parse(artifact['minimum_system_version'] as String);
          validateModuleAssetUri(uri);
          if (!{'macos', 'windows', 'android', 'linux'}.contains(platform) ||
              !{'x86', 'x86_64', 'arm', 'aarch64'}.contains(architecture) ||
              !targets.add('$platform/$architecture') ||
              !uri.path.endsWith('.cpmodule') ||
              size <= 0 ||
              size > maximumModulePackageBytes ||
              !RegExp(r'^[a-f0-9]{64}$').hasMatch(digest)) {
            throw const FormatException();
          }
          artifactPlatforms.add(platform);
          if (platform == target.platform &&
              architecture == target.architecture) {
            selected = ModuleArtifact(
              downloadUri: uri,
              sizeBytes: size,
              sha256: digest,
              minimumSystemVersion: minimumSystemVersion,
            );
          }
        }
        final rawPlatforms = entry['supported_platforms'];
        final platformNames = rawPlatforms == null
            ? artifactPlatforms.toList()
            : (rawPlatforms as List<dynamic>).cast<String>();
        if (rawPlatforms != null &&
            (platformNames.isEmpty ||
                platformNames.length != platformNames.toSet().length ||
                platformNames
                    .toSet()
                    .difference(artifactPlatforms)
                    .isNotEmpty ||
                artifactPlatforms
                    .difference(platformNames.toSet())
                    .isNotEmpty)) {
          throw const FormatException();
        }
        final supportedPlatforms = List<ModulePlatform>.unmodifiable(
          platformNames.map(ModulePlatform.values.byName),
        );
        var availability = ModuleAvailability.available;
        String? reason;
        final minimum = selected?.minimumSystemVersion;
        final strings = AppStrings.current;
        final systemName = switch (target.platform) {
          'macos' => 'macOS',
          'windows' => 'Windows',
          'android' => 'Android',
          'linux' => 'glibc',
          _ => strings.modulesThisSystem,
        };
        final systemRequirement = minimum == null
            ? null
            : strings.modulesSystemRequirement(
                systemName,
                minimum.patch != 0
                    ? minimum.toString()
                    : minimum.minor != 0
                    ? '${minimum.major}.${minimum.minor}'
                    : minimum.major.toString(),
              );
        if (selected == null) {
          availability = ModuleAvailability.platform;
          reason = strings.modulesNotAvailableForDevice;
        } else if (target.appVersion == null ||
            !compatibility.allows(target.appVersion!)) {
          availability = ModuleAvailability.appVersion;
          reason = target.appVersion == null
              ? strings.modulesAppVersionUnverified
              : strings.modulesAppVersionUnsupported(
                  target.appVersion.toString(),
                );
        } else if (minimum != null &&
            (target.systemVersion == null || target.systemVersion! < minimum)) {
          availability = ModuleAvailability.systemVersion;
          reason = strings.modulesRequires(systemRequirement!);
        }
        modules.add(
          MarketplaceModule(
            id: id,
            title: title,
            description: description,
            version: version,
            artifact: selected,
            supportedPlatforms: supportedPlatforms,
            appVersions: compatibility,
            availability: availability,
            unavailableReason: reason,
            systemRequirement: systemRequirement,
          ),
        );
      }
      modules.sort(
        (a, b) => a.title.toLowerCase().compareTo(b.title.toLowerCase()),
      );
      return List.unmodifiable(modules);
    } on ModulesException {
      rethrow;
    } on FormatException {
      throw ModulesException(AppStrings.current.modulesCatalogInvalid);
    } on TypeError {
      throw ModulesException(AppStrings.current.modulesCatalogInvalid);
    } on ArgumentError {
      throw ModulesException(AppStrings.current.modulesCatalogInvalid);
    }
  }
}
