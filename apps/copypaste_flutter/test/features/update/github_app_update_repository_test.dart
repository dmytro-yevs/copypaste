import 'dart:convert';
import 'dart:io';

import 'package:copypaste_flutter/features/update/update.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pub_semver/pub_semver.dart';

void main() {
  test(
    'stable checks fetch only the latest release and retain its installer',
    () async {
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      final paths = <String>[];
      server.listen((request) async {
        paths.add(request.uri.toString());
        request.response.write(jsonEncode(_release('1.0.35')));
        await request.response.close();
      });
      final repository = GitHubAppUpdateRepository(
        temporaryDirectory: () async =>
            throw const FileSystemException('unavailable'),
        releasesUri: Uri.parse(
          'http://127.0.0.1:${server.port}/releases?per_page=50',
        ),
      );
      addTearDown(repository.dispose);
      addTearDown(() => server.close(force: true));
      final release = await repository.findUpdate(
        currentVersion: Version.parse('1.0.34'),
        target: AppUpdateTarget.windows,
      );
      expect(paths, ['/releases/latest']);
      expect(release?.version, Version.parse('1.0.35'));
      expect(
        release?.asset?.name,
        'CopyPaste-v1.0.35-windows-x86_64-setup.exe',
      );
      expect(release?.asset?.signatureSha256, 'a' * 64);
    },
  );

  test('prerelease checks retain channel discovery', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final paths = <String>[];
    server.listen((request) async {
      paths.add(request.uri.toString());
      request.response.write(
        jsonEncode([
          _release('1.0.35-rc.2')..['prerelease'] = true,
          _release('1.0.34'),
        ]),
      );
      await request.response.close();
    });
    final repository = GitHubAppUpdateRepository(
      temporaryDirectory: () async =>
          throw const FileSystemException('unavailable'),
      releasesUri: Uri.parse(
        'http://127.0.0.1:${server.port}/releases?per_page=50',
      ),
    );
    addTearDown(repository.dispose);
    addTearDown(() => server.close(force: true));
    final release = await repository.findUpdate(
      currentVersion: Version.parse('1.0.35-rc.1'),
      target: AppUpdateTarget.windows,
    );
    expect(paths, ['/releases?per_page=50']);
    expect(release?.version, Version.parse('1.0.35-rc.2'));
  });
}

Map<String, Object?> _release(String version) {
  final name = 'CopyPaste-v$version-windows-x86_64-setup.exe';
  return {
    'tag_name': 'v$version',
    'draft': false,
    'prerelease': false,
    'assets': [
      for (final asset in [name, '$name.sig'])
        {
          'name': asset,
          'size': 128,
          'digest': 'sha256:${'a' * 64}',
          'browser_download_url':
              'https://github.com/dmytro-yevs/copypaste/releases/download/v$version/$asset',
        },
    ],
  };
}
