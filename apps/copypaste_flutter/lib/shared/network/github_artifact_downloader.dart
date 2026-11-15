import 'dart:async';
import 'dart:io';

import 'package:crypto/crypto.dart';

const githubReleaseAssetPath = '/dmytro-yevs/copypaste/releases/download/';

enum ArtifactDownloadFailure {
  untrustedUrl,
  untrustedRedirect,
  redirects,
  unavailable,
  tooLarge,
  integrity,
}

class ArtifactDownloadException implements Exception {
  const ArtifactDownloadException(this.failure, {this.statusCode});
  final ArtifactDownloadFailure failure;
  final int? statusCode;
}

/// Shared release-asset transport. The caller owns the client and cache policy.
class GitHubArtifactDownloader {
  GitHubArtifactDownloader(this._client, {required this.userAgent});
  final HttpClient _client;
  final String userAgent;
  static const _timeout = Duration(seconds: 30);

  static void validateAssetUri(Uri uri) {
    if (!_trustedAuthority(uri) ||
        uri.host != 'github.com' ||
        uri.hasQuery ||
        uri.hasFragment ||
        !uri.path.startsWith(githubReleaseAssetPath) ||
        uri.pathSegments.any((part) => part == '.' || part == '..')) {
      throw const ArtifactDownloadException(
        ArtifactDownloadFailure.untrustedUrl,
      );
    }
  }

  static bool _trustedAuthority(Uri uri) =>
      uri.scheme == 'https' && uri.port == 443 && uri.userInfo.isEmpty;
  static void _validateRedirect(Uri uri) {
    if (!_trustedAuthority(uri) ||
        !{
          'github.com',
          'objects.githubusercontent.com',
          'release-assets.githubusercontent.com',
        }.contains(uri.host)) {
      throw const ArtifactDownloadException(
        ArtifactDownloadFailure.untrustedRedirect,
      );
    }
  }

  Future<HttpClientResponse?> _open(
    Uri initialUri, {
    required bool allowMissing,
  }) async {
    validateAssetUri(initialUri);
    var uri = initialUri;
    for (var redirects = 0; redirects <= 5; redirects++) {
      _validateRedirect(uri);
      final request = await _client.getUrl(uri).timeout(_timeout);
      request.followRedirects = false;
      request.headers.set(HttpHeaders.userAgentHeader, userAgent);
      final response = await request.close().timeout(_timeout);
      if ({301, 302, 303, 307, 308}.contains(response.statusCode)) {
        final location = response.headers.value(HttpHeaders.locationHeader);
        await response.drain<void>().timeout(_timeout);
        if (location == null) {
          throw const ArtifactDownloadException(
            ArtifactDownloadFailure.redirects,
          );
        }
        uri = uri.resolve(location);
        continue;
      }
      if (response.statusCode != HttpStatus.ok) {
        request.abort();
        if (allowMissing && response.statusCode == HttpStatus.notFound) {
          return null;
        }
        throw ArtifactDownloadException(
          ArtifactDownloadFailure.unavailable,
          statusCode: response.statusCode,
        );
      }
      return response;
    }
    throw const ArtifactDownloadException(ArtifactDownloadFailure.redirects);
  }

  /// Streams with sink backpressure and hashes the received bytes in the same pass.
  /// Failed partial files are removed; atomic cache publication belongs to callers.
  Future<bool> download(
    Uri uri,
    File destination, {
    required int maximumBytes,
    int? expectedSize,
    String? expectedSha256,
    void Function(double)? onProgress,
    bool allowMissing = false,
  }) async {
    final response = await _open(uri, allowMissing: allowMissing);
    if (response == null) return false;
    if (response.contentLength > maximumBytes ||
        (expectedSize != null &&
            response.contentLength >= 0 &&
            response.contentLength != expectedSize)) {
      await response.listen((_) {}).cancel();
      throw const ArtifactDownloadException(ArtifactDownloadFailure.tooLarge);
    }
    final output = destination.openWrite();
    output.done.ignore();
    final digest = _DigestSink();
    final hash = sha256.startChunkedConversion(digest);
    var received = 0;
    var hashClosed = false;
    try {
      await output.addStream(
        response.timeout(_timeout).map((chunk) {
          received += chunk.length;
          if (received > maximumBytes ||
              (expectedSize != null && received > expectedSize)) {
            throw const ArtifactDownloadException(
              ArtifactDownloadFailure.tooLarge,
            );
          }
          hash.add(chunk);
          onProgress?.call(received / (expectedSize ?? maximumBytes));
          return chunk;
        }),
      );
      await output.flush();
      await output.close();
      hash.close();
      hashClosed = true;
      if ((expectedSize != null && received != expectedSize) ||
          (expectedSha256 != null &&
              digest.value.toString() != expectedSha256)) {
        throw const ArtifactDownloadException(
          ArtifactDownloadFailure.integrity,
        );
      }
      return true;
    } catch (_) {
      await output.close().catchError((Object _) {});
      if (await destination.exists()) await destination.delete();
      rethrow;
    } finally {
      if (!hashClosed) hash.close();
    }
  }

  Future<bool> matchesDigest(
    File file, {
    required String expectedSha256,
    required int expectedSize,
  }) async {
    if (await file.length() != expectedSize) return false;
    return (await sha256.bind(file.openRead()).first).toString() ==
        expectedSha256;
  }
}

class _DigestSink implements Sink<Digest> {
  Digest? value;
  @override
  void add(Digest data) => value = data;
  @override
  void close() {}
}
