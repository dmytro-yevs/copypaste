import 'dart:convert';
import 'dart:ffi';

import 'package:copypaste_flutter/features/update/update.dart';
import 'package:copypaste_flutter/platform/update/app_update_architecture.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pub_semver/pub_semver.dart';

void main() {
  const parser = GitHubReleaseParser();

  test('stable installations ignore prereleases', () {
    final body = jsonEncode([
      _release('1.1.0-rc.1', prerelease: true, target: AppUpdateTarget.windows),
      _release('1.0.1', prerelease: false, target: AppUpdateTarget.windows),
    ]);

    final release = parser.latestFor(
      body,
      currentVersion: Version.parse('1.0.0'),
      target: AppUpdateTarget.windows,
    );

    expect(release?.version, Version.parse('1.0.1'));
    expect(release?.asset?.name, 'CopyPaste-v1.0.1-windows-x86_64-setup.exe');
  });

  test('prerelease installations receive the next prerelease', () {
    final body = jsonEncode([
      _release('1.1.0-rc.2', prerelease: true, target: AppUpdateTarget.android),
      _release('1.1.0-rc.0', prerelease: true, target: AppUpdateTarget.android),
    ]);

    final release = parser.latestFor(
      body,
      currentVersion: Version.parse('1.1.0-rc.1'),
      target: AppUpdateTarget.android,
    );

    expect(release?.version, Version.parse('1.1.0-rc.2'));
    expect(release?.asset?.sha256, 'a' * 64);
  });

  test('downloadable platforms reject a release without a GitHub digest', () {
    final raw = _release(
      '1.0.1',
      prerelease: false,
      target: AppUpdateTarget.android,
    );
    final asset = (raw['assets'] as List<Object?>).first;
    (asset as Map<String, Object?>).remove('digest');

    final release = parser.latestFor(
      jsonEncode([raw]),
      currentVersion: Version.parse('1.0.0'),
      target: AppUpdateTarget.android,
    );

    expect(release, isNull);
  });

  test('macOS accepts the release without a downloadable artifact', () {
    final release = parser.latestFor(
      jsonEncode([
        _release('1.0.1', prerelease: false, target: AppUpdateTarget.android),
      ]),
      currentVersion: Version.parse('1.0.0'),
      target: AppUpdateTarget.macos,
    );

    expect(release?.version, Version.parse('1.0.1'));
    expect(release?.asset, isNull);
  });

  test('Android process ABI selects ARM variants or universal', () {
    expect(
      currentAndroidUpdateArchitecture(abi: Abi.androidArm64),
      AndroidAppUpdateArchitecture.arm64,
    );
    expect(
      currentAndroidUpdateArchitecture(abi: Abi.androidArm),
      AndroidAppUpdateArchitecture.armv7,
    );
    for (final abi in [Abi.androidX64, Abi.androidIA32, Abi.macosArm64]) {
      expect(currentAndroidUpdateArchitecture(abi: abi), isNull);
    }
  });

  for (final architecture in AndroidAppUpdateArchitecture.values) {
    test('Android ${architecture.name} prefers its signed APK', () {
      final raw = _androidVariants('1.0.1');
      final release = parser.latestFor(
        jsonEncode([raw]),
        currentVersion: Version.parse('1.0.0'),
        target: AppUpdateTarget.android,
        androidArchitecture: architecture,
      );
      final name = 'CopyPaste-v1.0.1-android-${architecture.name}.apk';
      expect(release?.asset?.name, name);
      expect(release?.asset?.signatureUri.path, endsWith('/$name.sig'));
    });

    test(
      'Android ${architecture.name} falls back to older universal releases',
      () {
        final release = parser.latestFor(
          jsonEncode([
            _release(
              '1.0.1',
              prerelease: false,
              target: AppUpdateTarget.android,
            ),
          ]),
          currentVersion: Version.parse('1.0.0'),
          target: AppUpdateTarget.android,
          androidArchitecture: architecture,
        );
        expect(release?.asset?.name, 'CopyPaste-v1.0.1-android.apk');
      },
    );

    test(
      'Android ${architecture.name} requires its own signature and digest',
      () {
        for (final invalidPart in ['signature', 'digest']) {
          final raw = _androidVariants('1.0.1');
          final name = 'CopyPaste-v1.0.1-android-${architecture.name}.apk';
          final assets = raw['assets']! as List<Map<String, Object?>>;
          if (invalidPart == 'signature') {
            assets.removeWhere((asset) => asset['name'] == '$name.sig');
          } else {
            assets
                .firstWhere((asset) => asset['name'] == name)
                .remove('digest');
          }
          final release = parser.latestFor(
            jsonEncode([raw]),
            currentVersion: Version.parse('1.0.0'),
            target: AppUpdateTarget.android,
            androidArchitecture: architecture,
          );
          expect(release?.asset?.name, 'CopyPaste-v1.0.1-android.apk');
          assets.removeWhere(
            (asset) => (asset['name']! as String).startsWith(
              'CopyPaste-v1.0.1-android.apk',
            ),
          );
          expect(
            parser.latestFor(
              jsonEncode([raw]),
              currentVersion: Version.parse('1.0.0'),
              target: AppUpdateTarget.android,
              androidArchitecture: architecture,
            ),
            isNull,
          );
        }
      },
    );
  }

  test('Android without a shipped ARM architecture uses universal', () {
    final release = parser.latestFor(
      jsonEncode([_androidVariants('1.0.1')]),
      currentVersion: Version.parse('1.0.0'),
      target: AppUpdateTarget.android,
    );
    expect(release?.asset?.name, 'CopyPaste-v1.0.1-android.apk');
  });

  test('rejects malformed GitHub metadata with a bounded update error', () {
    expect(
      () => parser.latestFor(
        '{',
        currentVersion: Version.parse('1.0.0'),
        target: AppUpdateTarget.android,
      ),
      throwsA(isA<AppUpdateException>()),
    );
  });
}

Map<String, Object?> _androidVariants(String version) {
  final raw = _release(
    version,
    prerelease: false,
    target: AppUpdateTarget.android,
  );
  final universal = (raw['assets']! as List<Map<String, Object?>>).toList();
  raw['assets'] = [
    ...universal,
    for (final architecture in AndroidAppUpdateArchitecture.values)
      for (final asset in universal)
        {
          ...asset,
          'name': (asset['name']! as String).replaceFirst(
            '-android.apk',
            '-android-${architecture.name}.apk',
          ),
          'browser_download_url': (asset['browser_download_url']! as String)
              .replaceFirst(
                '-android.apk',
                '-android-${architecture.name}.apk',
              ),
        },
  ];
  return raw;
}

Map<String, Object?> _release(
  String version, {
  required bool prerelease,
  required AppUpdateTarget target,
}) {
  final name = switch (target) {
    AppUpdateTarget.windows => 'CopyPaste-v$version-windows-x86_64-setup.exe',
    AppUpdateTarget.android ||
    AppUpdateTarget.macos => 'CopyPaste-v$version-android.apk',
  };
  return {
    'tag_name': 'v$version',
    'html_url':
        'https://github.com/dmytro-yevs/copypaste/releases/tag/v$version',
    'draft': false,
    'prerelease': prerelease,
    'published_at': '2026-10-04T12:00:00Z',
    'assets': [
      {
        'name': name,
        'browser_download_url':
            'https://github.com/dmytro-yevs/copypaste/releases/download/v$version/$name',
        'digest': 'sha256:${'a' * 64}',
        'size': 1024,
      },
      {
        'name': '$name.sig',
        'browser_download_url':
            'https://github.com/dmytro-yevs/copypaste/releases/download/v$version/$name.sig',
        'digest': 'sha256:${'b' * 64}',
        'size': 512,
      },
    ],
  };
}
