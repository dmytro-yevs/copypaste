import 'dart:io';
import 'dart:convert';

import 'package:copypaste_flutter/platform/files/temporary_artifacts.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test(
    'another process lease is skipped promptly and reclaimed after exit',
    () async {
      final root = await Directory.systemTemp.createTemp(
        'temporary-artifacts-process-',
      );
      addTearDown(() => root.delete(recursive: true));
      final staging = await Directory(
        '${root.path}/copypaste-staging',
      ).create();
      final active = await staging.createTemp('copypaste-ocr-');
      final flutterRoot = Platform.environment['FLUTTER_ROOT'];
      final dart = flutterRoot == null
          ? 'dart'
          : '$flutterRoot/bin/cache/dart-sdk/bin/dart${Platform.isWindows ? '.exe' : ''}';
      final child = await Process.start(dart, [
        'test/fixtures/temporary_lease_child.dart',
        active.path,
      ]);
      try {
        expect(
          await child.stdout
              .transform(utf8.decoder)
              .transform(const LineSplitter())
              .first,
          'ready',
        );
        final future = DateTime.now().add(const Duration(days: 3));
        await TemporaryArtifactCleanup(
          temporaryDirectory: () async => root,
          now: () => future,
        ).run().timeout(const Duration(seconds: 2));
        expect(await active.exists(), isTrue);
        child.stdin.writeln('release');
        await child.stdin.close();
        expect(await child.exitCode, 0);
        await TemporaryArtifactCleanup(
          temporaryDirectory: () async => root,
          now: () => future,
        ).run();
        expect(await active.exists(), isFalse);
      } finally {
        child.kill();
      }
    },
  );
  test(
    'removes abandoned owned staging, preserves active leases and other files',
    () async {
      final root = await Directory.systemTemp.createTemp(
        'temporary-artifacts-test-',
      );
      addTearDown(() => root.delete(recursive: true));
      final future = DateTime.now().add(const Duration(days: 3));
      final staging = await Directory(
        '${root.path}/copypaste-staging',
      ).create();
      final abandoned = await staging.createTemp('copypaste-ocr-');
      final active = await staging.createTemp('module-download-');
      final unrelated = await root.createTemp('user-document-');
      final fresh = await staging.createTemp('copypaste-input-');
      final lease = await TemporaryDirectoryLease.acquire(active);
      await TemporaryArtifactCleanup(
        temporaryDirectory: () async => root,
      ).run();
      expect(await fresh.exists(), isTrue);
      await TemporaryArtifactCleanup(
        temporaryDirectory: () async => root,
        now: () => future,
      ).run();
      expect(await abandoned.exists(), isFalse);
      expect(await active.exists(), isTrue);
      expect(await unrelated.exists(), isTrue);
      await lease.dispose();
      expect(await active.exists(), isFalse);
    },
  );

  test(
    'reclaims a lease left by an exited process and never follows symlinks',
    () async {
      final root = await Directory.systemTemp.createTemp(
        'temporary-artifacts-crash-',
      );
      addTearDown(() => root.delete(recursive: true));
      final staging = await Directory(
        '${root.path}/copypaste-staging',
      ).create();
      final abandoned = await staging.createTemp('copypaste-transfer-');
      await File('${abandoned.path}/.copypaste-owner').writeAsString('-1');
      await File('${abandoned.path}/.copypaste-active').writeAsString('');
      final protected = await root.createTemp('external-');
      await File('${protected.path}/original').writeAsString('preserve');
      if (!Platform.isWindows) {
        await Link(
          '${staging.path}/copypaste-ocr-symlink',
        ).create(protected.path);
      }
      await TemporaryArtifactCleanup(
        temporaryDirectory: () async => root,
        now: () => DateTime.now().add(const Duration(days: 3)),
      ).run();
      expect(await abandoned.exists(), isFalse);
      expect(
        await File('${protected.path}/original').readAsString(),
        'preserve',
      );
    },
  );
}
