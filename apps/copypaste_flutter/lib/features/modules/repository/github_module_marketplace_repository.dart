import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:pub_semver/pub_semver.dart';

import '../../../shared/security/minisign_verifier.dart';
import '../models/module_marketplace_models.dart';
import '../models/module_models.dart';
import 'module_marketplace_repository.dart';
import 'modules_repository.dart';

const moduleRepositoryPath = '/dmytro-yevs/copypaste/releases/download/';
const moduleCatalogName = 'modules.json';
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
  static const _timeout = Duration(seconds: 30);
  final Future<Directory> Function() _temporaryDirectory;
  final Future<ModuleMarketplaceTarget> Function() _currentTarget;
  final HttpClient _client;
  final MinisignVerifier _signatureVerifier;

  @override
  Future<List<MarketplaceModule>> list() async {
    final directory = await _createStagingDirectory('module-catalog-');
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
        throw const ModulesException(
          'The module catalog signature is invalid.',
        );
      }
      return const ModuleCatalogParser().parse(
        await catalog.readAsString(),
        await _currentTarget(),
      );
    } on SocketException {
      throw const ModulesException(
        'Could not connect to the module marketplace.',
      );
    } on TimeoutException {
      throw const ModulesException('The module marketplace did not respond.');
    } finally {
      await directory.delete(recursive: true);
    }
  }

  @override
  Future<SelectedModulePackage> download(
    MarketplaceModule module, {
    required void Function(double progress) onProgress,
  }) async {
    final artifact = module.artifact;
    if (artifact == null || !module.canInstall) {
      throw const ModulesException(
        'This module is not compatible with this device.',
      );
    }
    validateModuleAssetUri(artifact.downloadUri);
    if (artifact.sizeBytes <= 0 ||
        artifact.sizeBytes > maximumModulePackageBytes) {
      throw const ModulesException('The module package size is invalid.');
    }
    final directory = await _createStagingDirectory('module-download-');
    try {
      final file = File('${directory.path}/package.cpmodule');
      await _download(
        artifact.downloadUri,
        file,
        maximumBytes: artifact.sizeBytes,
        onProgress: onProgress,
      );
      if (await file.length() != artifact.sizeBytes ||
          (await sha256.bind(file.openRead()).first).toString() !=
              artifact.sha256) {
        throw const ModulesException(
          'The module package failed its integrity check.',
        );
      }
      return SelectedModulePackage(
        path: file.path,
        dispose: () => directory.delete(recursive: true),
      );
    } catch (error) {
      await directory.delete(recursive: true);
      if (error is SocketException) {
        throw const ModulesException('The module download was interrupted.');
      }
      if (error is TimeoutException) {
        throw const ModulesException('The module download did not respond.');
      }
      rethrow;
    }
  }

  Future<Directory> _createStagingDirectory(String prefix) async {
    final root = await _temporaryDirectory();
    await root.create(recursive: true);
    return root.createTemp(prefix);
  }

  Future<bool> _download(
    Uri uri,
    File destination, {
    required int maximumBytes,
    void Function(double progress)? onProgress,
    bool allowMissing = false,
  }) async {
    validateModuleAssetUri(uri);
    for (var redirects = 0; redirects <= 5; redirects++) {
      if (uri.scheme != 'https' ||
          uri.userInfo.isNotEmpty ||
          uri.port != 443 ||
          !{
            'github.com',
            'objects.githubusercontent.com',
            'release-assets.githubusercontent.com',
          }.contains(uri.host)) {
        throw const ModulesException('The module download left GitHub.');
      }
      final request = await _client.getUrl(uri).timeout(_timeout);
      request.followRedirects = false;
      request.headers.set(HttpHeaders.userAgentHeader, 'CopyPaste modules');
      final response = await request.close().timeout(_timeout);
      if ({301, 302, 303, 307, 308}.contains(response.statusCode)) {
        final location = response.headers.value(HttpHeaders.locationHeader);
        await response.drain<void>().timeout(_timeout);
        if (location == null) break;
        uri = uri.resolve(location);
        continue;
      }
      if (response.statusCode != HttpStatus.ok) {
        request.abort();
        if (allowMissing && response.statusCode == HttpStatus.notFound) {
          return false;
        }
        throw ModulesException(
          response.statusCode == HttpStatus.notFound
              ? 'This module download is currently unavailable.'
              : 'The module marketplace is unavailable. Try again.',
        );
      }
      if (response.contentLength > maximumBytes) {
        request.abort();
        throw const ModulesException('The module download is too large.');
      }
      final output = destination.openWrite();
      var received = 0;
      try {
        await for (final chunk in response.timeout(_timeout)) {
          received += chunk.length;
          if (received > maximumBytes) {
            throw const ModulesException('The module download is too large.');
          }
          output.add(chunk);
          await output.flush();
          onProgress?.call(received / maximumBytes);
        }
        await output.flush();
      } finally {
        await output.close();
      }
      return true;
    }
    throw const ModulesException(
      'The module download used too many redirects.',
    );
  }

  @override
  void dispose() => _client.close(force: true);
}

void validateModuleAssetUri(Uri uri) {
  if (uri.scheme != 'https' ||
      uri.host != 'github.com' ||
      uri.port != 443 ||
      uri.userInfo.isNotEmpty ||
      uri.hasQuery ||
      uri.hasFragment ||
      !uri.path.startsWith(moduleRepositoryPath) ||
      uri.pathSegments.any((part) => part == '..' || part == '.')) {
    throw const ModulesException('The module package URL is not trusted.');
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
          if (!{'macos', 'windows', 'android'}.contains(platform) ||
              !{'x86', 'x86_64', 'arm', 'aarch64'}.contains(architecture) ||
              !targets.add('$platform/$architecture') ||
              !uri.path.endsWith('.cpmodule') ||
              size <= 0 ||
              size > maximumModulePackageBytes ||
              !RegExp(r'^[a-f0-9]{64}$').hasMatch(digest)) {
            throw const FormatException();
          }
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
        var availability = ModuleAvailability.available;
        String? reason;
        final minimum = selected?.minimumSystemVersion;
        final systemName = switch (target.platform) {
          'macos' => 'macOS',
          'windows' => 'Windows',
          'android' => 'Android',
          _ => 'this system',
        };
        final systemRequirement = minimum == null
            ? null
            : '$systemName ${minimum.patch != 0
                  ? minimum.toString()
                  : minimum.minor != 0
                  ? '${minimum.major}.${minimum.minor}'
                  : minimum.major.toString()} or newer';
        if (selected == null) {
          availability = ModuleAvailability.platform;
          reason = 'Not available for this device.';
        } else if (target.appVersion == null ||
            !compatibility.allows(target.appVersion!)) {
          availability = ModuleAvailability.appVersion;
          reason = target.appVersion == null
              ? 'The installed CopyPaste version could not be verified.'
              : 'This module does not support CopyPaste ${target.appVersion}.';
        } else if (minimum != null &&
            (target.systemVersion == null || target.systemVersion! < minimum)) {
          availability = ModuleAvailability.systemVersion;
          reason = 'Requires $systemRequirement.';
        }
        modules.add(
          MarketplaceModule(
            id: id,
            title: title,
            description: description,
            version: version,
            artifact: selected,
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
      throw const ModulesException('The module catalog is invalid.');
    } on TypeError {
      throw const ModulesException('The module catalog is invalid.');
    } on ArgumentError {
      throw const ModulesException('The module catalog is invalid.');
    }
  }
}
