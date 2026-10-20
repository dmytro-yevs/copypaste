import 'dart:io';

import 'package:copypaste_flutter/features/update/repository/obsolete_update_cleanup.dart';
import 'package:copypaste_flutter/features/update/repository/github_app_update_repository.dart';
import 'package:copypaste_flutter/features/update/models/app_update_models.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pub_semver/pub_semver.dart';

void main() {
  test(
    'temporary-directory failure does not prevent checking releases',
    () async {
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      server.listen((request) async {
        request.response.write('[]');
        await request.response.close();
      });
      final repository = GitHubAppUpdateRepository(
        temporaryDirectory: () async =>
            throw const FileSystemException('unavailable'),
        releasesUri: Uri.parse('http://127.0.0.1:${server.port}/releases'),
      );
      try {
        expect(
          await repository.findUpdate(
            currentVersion: Version.parse('1.0.29'),
            target: AppUpdateTarget.android,
          ),
          isNull,
        );
      } finally {
        repository.dispose();
        await server.close(force: true);
      }
    },
  );
  test(
    'removes installed packages and sidecars while preserving pending updates',
    () async {
      final root = await Directory.systemTemp.createTemp(
        'update-cleanup-test-',
      );
      addTearDown(() => root.delete(recursive: true));
      const obsolete = [
        'CopyPaste-v1.0.28-android.apk',
        'CopyPaste-v1.0.28-android-arm64.apk.sig',
        'CopyPaste-v1.0.29-windows-x86_64-setup.exe',
        'CopyPaste-v1.0.29-android-armv7.apk.part',
        'CopyPaste-v1.0.29-linux-aarch64.deb',
        'CopyPaste-v1.0.29-rc.1-android.apk',
      ];
      const retained = [
        'CopyPaste-v1.0.30-android.apk',
        'CopyPaste-v1.0.30-android.apk.sig',
        'CopyPaste-v1.0.30-android.apk.part',
        'user-file.apk',
        'CopyPaste-vinvalid-android.apk',
      ];
      for (final name in [...obsolete, ...retained]) {
        await File('${root.path}/$name').writeAsString('fixture');
      }
      await removeInstalledUpdatePackages(root, Version.parse('1.0.29'));
      for (final name in obsolete) {
        expect(
          await File('${root.path}/$name').exists(),
          isFalse,
          reason: name,
        );
      }
      for (final name in retained) {
        expect(await File('${root.path}/$name').exists(), isTrue, reason: name);
      }
    },
  );
}
