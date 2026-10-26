import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:copypaste_flutter/shared/network/github_artifact_downloader.dart';
import 'package:crypto/crypto.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final asset = Uri.parse(
    'https://github.com${githubReleaseAssetPath}v1/package',
  );
  late Directory directory;
  late File destination;
  late _Client client;
  late GitHubArtifactDownloader downloader;
  setUp(() async {
    directory = await Directory.systemTemp.createTemp('artifact-download-');
    destination = File('${directory.path}/package.part');
    client = _Client();
    downloader = GitHubArtifactDownloader(client, userAgent: 'CopyPaste test');
  });
  tearDown(() => directory.delete(recursive: true));

  test(
    'both consumers get exact bytes and a digest checked during streaming',
    () async {
      final bytes = utf8.encode('authenticated package');
      final cdn = Uri.parse(
        'https://release-assets.githubusercontent.com/file?token=opaque',
      );
      client.routes[asset] = _Response(
        [],
        statusCode: 302,
        location: cdn.toString(),
      );
      client.routes[cdn] = _Response([bytes.sublist(0, 4), bytes.sublist(4)]);
      final progress = <double>[];
      expect(
        await downloader.download(
          asset,
          destination,
          maximumBytes: bytes.length,
          expectedSize: bytes.length,
          expectedSha256: sha256.convert(bytes).toString(),
          onProgress: progress.add,
        ),
        isTrue,
      );
      expect(await destination.readAsBytes(), bytes);
      expect(progress.last, 1);
      expect(client.requests, [asset, cdn]);
      expect(client.followRedirects, everyElement(isFalse));
    },
  );

  test(
    'untrusted initial authorities and redirects never receive a request',
    () async {
      for (final uri in [
        asset.replace(userInfo: 'user:password'),
        asset.replace(port: 444),
        asset.replace(query: 'redirect=other'),
        asset.replace(fragment: 'other'),
        Uri.parse('http://github.com${asset.path}'),
      ]) {
        await expectLater(
          downloader.download(uri, destination, maximumBytes: 10),
          throwsA(isA<ArtifactDownloadException>()),
        );
      }
      expect(client.requests, isEmpty);
      for (final location in [
        'https://example.com/file',
        'https://user:password@github.com/file',
        'https://github.com:444/file',
      ]) {
        client.routes[asset] = _Response(
          [],
          statusCode: 302,
          location: location,
        );
        await expectLater(
          downloader.download(asset, destination, maximumBytes: 10),
          throwsA(isA<ArtifactDownloadException>()),
        );
        expect(client.requests.last, asset);
      }
      expect(await destination.exists(), isFalse);
    },
  );

  test(
    'unknown-length oversized, truncated, wrong-digest and interrupted streams remove partial files',
    () async {
      final cases = [
        _Response([
          [1, 2],
          [3, 4],
        ], contentLength: -1),
        _Response([
          [1],
        ], contentLength: -1),
        _Response([
          [2, 2, 2],
        ], contentLength: -1),
        _Response(
          [
            [1],
          ],
          contentLength: -1,
          failure: const SocketException('interrupted'),
        ),
      ];
      for (final response in cases) {
        client.routes[asset] = response;
        await expectLater(
          downloader.download(
            asset,
            destination,
            maximumBytes: 3,
            expectedSize: 3,
            expectedSha256: sha256.convert([1, 2, 3]).toString(),
          ),
          throwsA(isA<Object>()),
        );
        expect(await destination.exists(), isFalse);
      }
    },
  );

  test('only an explicitly optional 404 is treated as missing', () async {
    client.routes[asset] = _Response([], statusCode: 404);
    expect(
      await downloader.download(
        asset,
        destination,
        maximumBytes: 10,
        allowMissing: true,
      ),
      isFalse,
    );
    await expectLater(
      downloader.download(asset, destination, maximumBytes: 10),
      throwsA(isA<ArtifactDownloadException>()),
    );
    expect(await destination.exists(), isFalse);
  });

  test('redirect loops stop at the shared limit', () async {
    client.routes[asset] = _Response(
      [],
      statusCode: 302,
      location: asset.toString(),
    );
    await expectLater(
      downloader.download(asset, destination, maximumBytes: 10),
      throwsA(isA<ArtifactDownloadException>()),
    );
    expect(client.requests, hasLength(6));
  });
}

class _Client implements HttpClient {
  final routes = <Uri, _Response>{};
  final requests = <Uri>[];
  final followRedirects = <bool>[];
  @override
  Future<HttpClientRequest> getUrl(Uri url) async {
    requests.add(url);
    return _Request(routes[url]!, followRedirects);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Request implements HttpClientRequest {
  _Request(this.response, this.redirectSettings);
  final _Response response;
  final List<bool> redirectSettings;
  @override
  final HttpHeaders headers = _Headers();
  @override
  set followRedirects(bool value) => redirectSettings.add(value);
  @override
  Future<HttpClientResponse> close() async => response;
  @override
  void abort([Object? exception, StackTrace? stackTrace]) {}
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Headers implements HttpHeaders {
  _Headers([this.location]);
  final String? location;
  @override
  void set(String name, Object value, {bool preserveHeaderCase = false}) {}
  @override
  String? value(String name) =>
      name == HttpHeaders.locationHeader ? location : null;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Response extends Stream<List<int>> implements HttpClientResponse {
  _Response(
    this.chunks, {
    this.statusCode = 200,
    this.contentLength = -1,
    String? location,
    this.failure,
  }) : headers = _Headers(location);
  final List<List<int>> chunks;
  final Object? failure;
  @override
  final int statusCode;
  @override
  final int contentLength;
  @override
  final HttpHeaders headers;
  Stream<List<int>> _stream() async* {
    for (final chunk in chunks) {
      yield chunk;
    }
    if (failure != null) {
      throw failure!;
    }
  }

  @override
  StreamSubscription<List<int>> listen(
    void Function(List<int>)? onData, {
    Function? onError,
    void Function()? onDone,
    bool? cancelOnError,
  }) => _stream().listen(
    onData,
    onError: onError,
    onDone: onDone,
    cancelOnError: cancelOnError,
  );
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
