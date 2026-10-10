import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:pub_semver/pub_semver.dart';

import '../../../platform/update/app_update_architecture.dart';
import '../models/app_update_models.dart';
import 'app_update_repository.dart';
import 'minisign_verifier.dart';

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
    final request = await _client.getUrl(_releasesUri).timeout(_requestTimeout);
    request.headers
      ..set(HttpHeaders.acceptHeader, 'application/vnd.github+json')
      ..set(HttpHeaders.userAgentHeader, 'CopyPaste updater')
      ..set('X-GitHub-Api-Version', '2026-03-10');
    final response = await request.close().timeout(_requestTimeout);
    if (response.statusCode != HttpStatus.ok) {
      await response.drain<void>();
      throw const AppUpdateException('CopyPaste could not check for updates.');
    }
    final body = await _readBoundedUtf8(
      response,
      maximumBytes: _maximumMetadataBytes,
    );
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
        await _matchesDigest(
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
    final response = await _openDownload(asset.downloadUri);
    final contentLength = response.contentLength;
    if (contentLength > _maximumArtifactBytes ||
        (contentLength > 0 && contentLength != asset.sizeBytes)) {
      await response.drain<void>();
      throw const AppUpdateException('The update package size is invalid.');
    }

    final output = partial.openWrite();
    final digestSink = _DigestSink();
    final digestInput = sha256.startChunkedConversion(digestSink);
    var received = 0;
    try {
      await for (final chunk in response.timeout(_requestTimeout)) {
        received += chunk.length;
        if (received > _maximumArtifactBytes || received > asset.sizeBytes) {
          throw const AppUpdateException('The update package is too large.');
        }
        output.add(chunk);
        digestInput.add(chunk);
        onProgress(received / asset.sizeBytes);
      }
      await output.flush();
      await output.close();
      digestInput.close();
    } catch (_) {
      await output.close().catchError((Object _) {});
      digestInput.close();
      if (await partial.exists()) await partial.delete();
      rethrow;
    }

    final digest = digestSink.value?.toString();
    if (received != asset.sizeBytes || digest != asset.sha256) {
      if (await partial.exists()) await partial.delete();
      throw const AppUpdateException(
        'The downloaded update failed its integrity check.',
      );
    }
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

  Future<HttpClientResponse> _openDownload(Uri initialUri) async {
    var uri = initialUri;
    for (var redirects = 0; redirects <= 5; redirects++) {
      _validateDownloadUri(uri, firstRequest: redirects == 0);
      final request = await _client.getUrl(uri).timeout(_requestTimeout);
      request
        ..followRedirects = false
        ..headers.set(HttpHeaders.userAgentHeader, 'CopyPaste updater');
      final response = await request.close().timeout(_requestTimeout);
      if (response.isRedirect) {
        final location = response.headers.value(HttpHeaders.locationHeader);
        await response.drain<void>();
        if (location == null) {
          throw const AppUpdateException(
            'The update download was redirected incorrectly.',
          );
        }
        uri = uri.resolve(location);
        continue;
      }
      if (response.statusCode != HttpStatus.ok) {
        await response.drain<void>();
        throw const AppUpdateException(
          'CopyPaste could not download the update.',
        );
      }
      return response;
    }
    throw const AppUpdateException(
      'The update download used too many redirects.',
    );
  }

  Future<bool> _matchesDigest(
    File file, {
    required String expectedSha256,
    required int expectedSize,
  }) async {
    if (await file.length() != expectedSize) return false;
    final digest = await sha256.bind(file.openRead()).first;
    return digest.toString() == expectedSha256;
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
        await _matchesDigest(
          destination,
          expectedSha256: asset.signatureSha256,
          expectedSize: asset.signatureSizeBytes,
        )) {
      return destination;
    }
    final response = await _openDownload(asset.signatureUri);
    final bytes = <int>[];
    await for (final chunk in response.timeout(_requestTimeout)) {
      if (bytes.length + chunk.length > _maximumSignatureBytes) {
        throw const AppUpdateException('The updater signature is too large.');
      }
      bytes.addAll(chunk);
    }
    if (bytes.length != asset.signatureSizeBytes ||
        sha256.convert(bytes).toString() != asset.signatureSha256) {
      throw const AppUpdateException(
        'The updater signature failed its integrity check.',
      );
    }
    await destination.writeAsBytes(bytes, flush: true);
    return destination;
  }

  void _validateReleaseAssetUri(Uri uri) {
    if (uri.scheme != 'https' ||
        uri.host != 'github.com' ||
        !uri.path.startsWith('/dmytro-yevs/copypaste/releases/download/')) {
      throw const AppUpdateException('The update package URL is not trusted.');
    }
  }

  void _validateDownloadUri(Uri uri, {required bool firstRequest}) {
    if (firstRequest) {
      _validateReleaseAssetUri(uri);
      return;
    }
    final trustedRedirect =
        uri.host == 'github.com' ||
        uri.host == 'objects.githubusercontent.com' ||
        uri.host == 'release-assets.githubusercontent.com';
    if (uri.scheme != 'https' || !trustedRedirect) {
      throw const AppUpdateException('The update download left GitHub.');
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
    if (decoded is! List<Object?>) {
      throw const AppUpdateException(
        'GitHub returned invalid update metadata.',
      );
    }
    AppRelease? selected;
    for (final rawRelease in decoded) {
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

class _DigestSink implements Sink<Digest> {
  Digest? value;

  @override
  void add(Digest data) => value = data;

  @override
  void close() {}
}
