import '../../../shared/network/github_artifact_downloader.dart';

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:pub_semver/pub_semver.dart';

import '../../../platform/update/app_update_architecture.dart';
import '../models/app_update_models.dart';
import 'app_update_repository.dart';
import 'minisign_verifier.dart';
import 'obsolete_update_cleanup.dart';

typedef UpdateTemporaryDirectoryProvider = Future<Directory> Function();

class GitHubAppUpdateRepository
    implements AppUpdateRepository, LinuxAppUpdateRepository {
  GitHubAppUpdateRepository({
    required UpdateTemporaryDirectoryProvider temporaryDirectory,
    HttpClient? client,
    Uri? releasesUri,
    AndroidAppUpdateArchitecture? Function()? androidArchitecture,
  }) : _temporaryDirectory = temporaryDirectory,
       _androidArchitecture =
           androidArchitecture ?? currentAndroidUpdateArchitecture,
       _client = client ?? HttpClient(),
       _releasesUri =
           releasesUri ??
           Uri.https(
             'api.github.com',
             '/repos/dmytro-yevs/copypaste/releases',
             const {'per_page': '50'},
           );

  static const _maximumMetadataBytes = 2 * 1024 * 1024;
  static const _maximumArtifactBytes = 512 * 1024 * 1024;
  static const _maximumSignatureBytes = 64 * 1024;
  static const _requestTimeout = Duration(seconds: 30);

  final UpdateTemporaryDirectoryProvider _temporaryDirectory;
  final AndroidAppUpdateArchitecture? Function() _androidArchitecture;
  final HttpClient _client;
  late final _downloader = GitHubArtifactDownloader(
    _client,
    userAgent: 'CopyPaste updater',
  );
  final Uri _releasesUri;
  final MinisignVerifier _signatureVerifier = MinisignVerifier(
    publicKeyBase64: copyPasteUpdaterPublicKey,
  );

  @override
  Future<AppRelease?> findUpdate({
    required Version currentVersion,
    required AppUpdateTarget target,
  }) => _findUpdate(currentVersion: currentVersion, target: target);

  @override
  Future<AppRelease?> findLinuxUpdate({
    required Version currentVersion,
    required LinuxAppUpdateInstallation installation,
  }) => _findUpdate(
    currentVersion: currentVersion,
    target: AppUpdateTarget.linux,
    linuxInstallation: installation,
  );

  Future<AppRelease?> _findUpdate({
    required Version currentVersion,
    required AppUpdateTarget target,
    LinuxAppUpdateInstallation? linuxInstallation,
  }) async {
    // Run before network access so installed packages are removed offline too.
    try {
      final temporary = await _temporaryDirectory();
      await removeInstalledUpdatePackages(
        Directory(
          '${temporary.path}${Platform.pathSeparator}copypaste-updates',
        ),
        currentVersion,
      );
    } catch (_) {
      // A missing temporary-directory provider must not break update checks.
    }
    // Stable installations need one release, not the history of fifty builds.
    // Prerelease installations still inspect the list to preserve their channel.
    final uri = currentVersion.isPreRelease
        ? _releasesUri
        : Uri(
            scheme: _releasesUri.scheme,
            userInfo: _releasesUri.userInfo,
            host: _releasesUri.host,
            port: _releasesUri.hasPort ? _releasesUri.port : null,
            path: '${_releasesUri.path}/latest',
          );
    final elapsed = Stopwatch()..start();
    final request = await _client.getUrl(uri).timeout(_requestTimeout);
    request.headers
      ..set(HttpHeaders.acceptHeader, 'application/vnd.github+json')
      ..set(HttpHeaders.userAgentHeader, 'CopyPaste updater')
      ..set('X-GitHub-Api-Version', '2026-03-10');
    final String body;
    try {
      body = await (() async {
        final response = await request.close();
        if (response.statusCode == HttpStatus.notFound &&
            !currentVersion.isPreRelease) {
          await response.drain<void>();
          return '[]';
        }
        if (response.statusCode != HttpStatus.ok) {
          await response.drain<void>();
          throw const AppUpdateException(
            'CopyPaste could not check for updates.',
          );
        }
        return _readBoundedUtf8(response, maximumBytes: _maximumMetadataBytes);
      })().timeout(_requestTimeout - elapsed.elapsed);
    } catch (_) {
      request.abort();
      rethrow;
    }
    return const GitHubReleaseParser().latestFor(
      body,
      currentVersion: currentVersion,
      target: target,
      androidArchitecture: target == AppUpdateTarget.android
          ? _androidArchitecture()
          : null,
      linuxInstallation: linuxInstallation,
    );
  }

  @override
  Future<DownloadedAppUpdate> download(
    AppRelease release, {
    required void Function(double progress) onProgress,
  }) async {
    final asset = release.asset;
    if (asset == null) {
      throw const AppUpdateException(
        'This platform does not use a downloadable update package.',
      );
    }
    _validateReleaseAssetUri(asset.downloadUri);
    if (asset.sizeBytes <= 0 || asset.sizeBytes > _maximumArtifactBytes) {
      throw const AppUpdateException('The update package size is invalid.');
    }

    final root = await _temporaryDirectory();
    final directory = Directory(
      '${root.path}${Platform.pathSeparator}copypaste-updates',
    );
    await directory.create(recursive: true);
    final destination = File(
      '${directory.path}${Platform.pathSeparator}${asset.name}',
    );
    if (await destination.exists() &&
        await _downloader.matchesDigest(
          destination,
          expectedSha256: asset.sha256,
          expectedSize: asset.sizeBytes,
        ) &&
        await _verifyUpdaterSignature(destination, asset, directory)) {
      onProgress(1);
      return DownloadedAppUpdate(path: destination.path, asset: asset);
    }

    final partial = File('${destination.path}.part');
    if (await partial.exists()) await partial.delete();
    await _downloadArtifact(
      asset.downloadUri,
      partial,
      maximumBytes: _maximumArtifactBytes,
      expectedSize: asset.sizeBytes,
      expectedSha256: asset.sha256,
      onProgress: onProgress,
    );
    if (await destination.exists()) await destination.delete();
    await partial.rename(destination.path);
    if (!await _verifyUpdaterSignature(destination, asset, directory)) {
      if (await destination.exists()) await destination.delete();
      throw const AppUpdateException(
        'The downloaded update has an invalid updater signature.',
      );
    }
    onProgress(1);
    return DownloadedAppUpdate(path: destination.path, asset: asset);
  }

  Future<void> _downloadArtifact(
    Uri uri,
    File destination, {
    required int maximumBytes,
    required int expectedSize,
    required String expectedSha256,
    void Function(double)? onProgress,
  }) async {
    try {
      await _downloader.download(
        uri,
        destination,
        maximumBytes: maximumBytes,
        expectedSize: expectedSize,
        expectedSha256: expectedSha256,
        onProgress: onProgress,
      );
    } on ArtifactDownloadException catch (error) {
      throw AppUpdateException(switch (error.failure) {
        ArtifactDownloadFailure.untrustedUrl =>
          'The update package URL is not trusted.',
        ArtifactDownloadFailure.untrustedRedirect =>
          'The update download left GitHub.',
        ArtifactDownloadFailure.redirects =>
          'The update download used too many redirects.',
        ArtifactDownloadFailure.tooLarge => 'The update package is too large.',
        ArtifactDownloadFailure.integrity =>
          'The downloaded update failed its integrity check.',
        ArtifactDownloadFailure.unavailable =>
          'CopyPaste could not download the update.',
      });
    }
  }

  Future<bool> _verifyUpdaterSignature(
    File package,
    AppReleaseAsset asset,
    Directory directory,
  ) async {
    final signature = await _downloadSignature(asset, directory);
    final valid = await _signatureVerifier.verifyFile(
      file: package,
      encodedSignature: await signature.readAsString(),
      expectedFileName: asset.name,
    );
    if (!valid && await signature.exists()) await signature.delete();
    return valid;
  }

  Future<File> _downloadSignature(
    AppReleaseAsset asset,
    Directory directory,
  ) async {
    _validateReleaseAssetUri(asset.signatureUri);
    if (asset.signatureSizeBytes <= 0 ||
        asset.signatureSizeBytes > _maximumSignatureBytes) {
      throw const AppUpdateException('The updater signature size is invalid.');
    }
    final destination = File(
      '${directory.path}${Platform.pathSeparator}${asset.name}.sig',
    );
    if (await destination.exists() &&
        await _downloader.matchesDigest(
          destination,
          expectedSha256: asset.signatureSha256,
          expectedSize: asset.signatureSizeBytes,
        )) {
      return destination;
    }
    final partial = File('${destination.path}.part');
    await _downloadArtifact(
      asset.signatureUri,
      partial,
      maximumBytes: _maximumSignatureBytes,
      expectedSize: asset.signatureSizeBytes,
      expectedSha256: asset.signatureSha256,
    );
    if (await destination.exists()) await destination.delete();
    await partial.rename(destination.path);
    return destination;
  }

  void _validateReleaseAssetUri(Uri uri) {
    try {
      GitHubArtifactDownloader.validateAssetUri(uri);
    } on ArtifactDownloadException {
      throw const AppUpdateException('The update package URL is not trusted.');
    }
  }

  @override
  void dispose() => _client.close(force: true);
}

class GitHubReleaseParser {
  const GitHubReleaseParser();

  static const _maximumArtifactBytes = 512 * 1024 * 1024;
  static final _sha256Pattern = RegExp(r'^sha256:([a-f0-9]{64})$');

  AppRelease? latestFor(
    String body, {
    required Version currentVersion,
    required AppUpdateTarget target,
    AndroidAppUpdateArchitecture? androidArchitecture,
    LinuxAppUpdateInstallation? linuxInstallation,
  }) {
    final Object? decoded;
    try {
      decoded = jsonDecode(body);
    } on FormatException {
      throw const AppUpdateException(
        'GitHub returned invalid update metadata.',
      );
    }
    final releases = switch (decoded) {
      List<Object?> values => values,
      Map<String, Object?> value => [value],
      _ => throw const AppUpdateException(
        'GitHub returned invalid update metadata.',
      ),
    };
    AppRelease? selected;
    for (final rawRelease in releases) {
      if (rawRelease is! Map<String, Object?> || rawRelease['draft'] == true) {
        continue;
      }
      final prerelease = rawRelease['prerelease'] == true;
      if (!currentVersion.isPreRelease && prerelease) continue;
      final tag = rawRelease['tag_name'] as String?;
      if (tag == null) continue;
      final versionText = tag.startsWith('v') ? tag.substring(1) : tag;
      final version = _tryParseVersion(versionText);
      if (version == null || version <= currentVersion) continue;

      final releaseUri = _releaseUri(rawRelease['html_url'], tag);
      final asset = target == AppUpdateTarget.macos
          ? null
          : _assetFor(
              rawRelease['assets'],
              target: target,
              version: versionText,
              androidArchitecture: androidArchitecture,
              linuxInstallation: linuxInstallation,
            );
      final releasePageOnly =
          target == AppUpdateTarget.linux && linuxInstallation == null;
      if (target != AppUpdateTarget.macos &&
          !releasePageOnly &&
          asset == null) {
        continue;
      }
      final candidate = AppRelease(
        version: version,
        releaseUri: releaseUri,
        prerelease: prerelease,
        publishedAt: DateTime.tryParse(
          rawRelease['published_at'] as String? ?? '',
        ),
        asset: asset,
      );
      if (selected == null || candidate.version > selected.version) {
        selected = candidate;
      }
    }
    return selected;
  }

  AppReleaseAsset? _assetFor(
    Object? rawAssets, {
    required AppUpdateTarget target,
    required String version,
    AndroidAppUpdateArchitecture? androidArchitecture,
    LinuxAppUpdateInstallation? linuxInstallation,
  }) {
    if (rawAssets is! List<Object?>) return null;
    if (target == AppUpdateTarget.linux && linuxInstallation == null) {
      return null;
    }
    final expectedName = switch (target) {
      AppUpdateTarget.windows => 'CopyPaste-v$version-windows-x86_64-setup.exe',
      AppUpdateTarget.android => 'CopyPaste-v$version-android.apk',
      AppUpdateTarget.macos => throw StateError('macOS uses Homebrew.'),
      AppUpdateTarget.linux => _linuxAssetName(
        version: version,
        installation: linuxInstallation,
      ),
    };
    final byName = <String, Map<String, Object?>>{
      for (final rawAsset in rawAssets)
        if (rawAsset is Map<String, Object?> && rawAsset['name'] is String)
          rawAsset['name']! as String: rawAsset,
    };
    final names = [
      if (target == AppUpdateTarget.android && androidArchitecture != null)
        'CopyPaste-v$version-android-${androidArchitecture.name}.apk',
      expectedName,
    ];
    for (final name in names) {
      final package = _assetMetadata(
        byName[name],
        maximumBytes: _maximumArtifactBytes,
      );
      final signature = _assetMetadata(
        byName['$name.sig'],
        maximumBytes: 64 * 1024,
      );
      if (package == null || signature == null) continue;
      return AppReleaseAsset(
        name: name,
        downloadUri: package.$1,
        sha256: package.$2,
        sizeBytes: package.$3,
        signatureUri: signature.$1,
        signatureSha256: signature.$2,
        signatureSizeBytes: signature.$3,
      );
    }
    return null;
  }

  String _linuxAssetName({
    required String version,
    required LinuxAppUpdateInstallation? installation,
  }) {
    if (installation == null) {
      throw StateError('Linux updates require the installed package details.');
    }
    final extension = switch (installation.package) {
      LinuxAppUpdatePackage.appImage => 'AppImage',
      LinuxAppUpdatePackage.deb => 'deb',
      LinuxAppUpdatePackage.rpm => 'rpm',
    };
    return 'CopyPaste-v$version-linux-${installation.architecture.name}.$extension';
  }

  (Uri, String, int)? _assetMetadata(
    Map<String, Object?>? raw, {
    required int maximumBytes,
  }) {
    if (raw == null) return null;
    final uri = Uri.tryParse(raw['browser_download_url'] as String? ?? '');
    final digestMatch = _sha256Pattern.firstMatch(
      raw['digest'] as String? ?? '',
    );
    final size = raw['size'];
    if (uri == null ||
        uri.scheme != 'https' ||
        uri.host != 'github.com' ||
        !uri.path.startsWith('/dmytro-yevs/copypaste/releases/download/') ||
        digestMatch == null ||
        size is! int ||
        size <= 0 ||
        size > maximumBytes) {
      return null;
    }
    return (uri, digestMatch.group(1)!, size);
  }

  Uri _releaseUri(Object? value, String tag) {
    final parsed = Uri.tryParse(value as String? ?? '');
    if (parsed != null &&
        parsed.scheme == 'https' &&
        parsed.host == 'github.com' &&
        parsed.port == 443 &&
        parsed.userInfo.isEmpty &&
        !parsed.pathSegments.any(
          (segment) => segment == '.' || segment == '..',
        ) &&
        parsed.path.startsWith('/dmytro-yevs/copypaste/releases/')) {
      return parsed;
    }
    return Uri.https('github.com', '/dmytro-yevs/copypaste/releases/tag/$tag');
  }
}

Version? _tryParseVersion(String value) {
  try {
    return Version.parse(value);
  } on FormatException {
    return null;
  }
}

Future<String> _readBoundedUtf8(
  Stream<List<int>> input, {
  required int maximumBytes,
}) async {
  final bytes = <int>[];
  await for (final chunk in input.timeout(const Duration(seconds: 30))) {
    if (bytes.length + chunk.length > maximumBytes) {
      throw const AppUpdateException('The update metadata is too large.');
    }
    bytes.addAll(chunk);
  }
  try {
    return utf8.decode(bytes);
  } on FormatException {
    throw const AppUpdateException('GitHub returned invalid update metadata.');
  }
}
