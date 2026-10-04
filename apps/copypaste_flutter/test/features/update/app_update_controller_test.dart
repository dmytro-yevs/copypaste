import 'package:copypaste_flutter/features/update/update.dart';
import 'package:copypaste_flutter/platform/update/app_update_platform.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pub_semver/pub_semver.dart';

void main() {
  test(
    'checks the installed channel and exposes an available update',
    () async {
      final repository = _FakeUpdateRepository(release: _release());
      final platform = _FakeUpdatePlatform();
      final controller = AppUpdateController(
        repository: repository,
        platform: platform,
      );
      addTearDown(controller.dispose);

      await controller.initialize();

      expect(controller.phase, AppUpdatePhase.available);
      expect(controller.currentVersion, Version.parse('1.0.0'));
      expect(controller.release?.version, Version.parse('1.0.1'));
      expect(repository.requestedTarget, AppUpdateTarget.android);
    },
  );

  test(
    'reuses the verified download after Android grants permission',
    () async {
      final repository = _FakeUpdateRepository(release: _release());
      final platform = _FakeUpdatePlatform(
        installResults: [
          AppUpdateInstallResult.permissionRequired,
          AppUpdateInstallResult.started,
        ],
      );
      final controller = AppUpdateController(
        repository: repository,
        platform: platform,
      );
      addTearDown(controller.dispose);
      await controller.initialize();

      await controller.install();
      expect(controller.phase, AppUpdatePhase.permissionRequired);
      await controller.install();

      expect(repository.downloadCalls, 1);
      expect(platform.installCalls, 2);
      expect(controller.phase, AppUpdatePhase.permissionRequired);
    },
  );

  test('macOS delegates installation to Homebrew without a download', () async {
    final repository = _FakeUpdateRepository(
      release: AppRelease(
        version: Version.parse('1.0.1'),
        releaseUri: Uri.parse(
          'https://github.com/dmytro-yevs/copypaste/releases/tag/v1.0.1',
        ),
        prerelease: false,
      ),
    );
    final platform = _FakeUpdatePlatform(
      target: AppUpdateTarget.macos,
      installResults: [AppUpdateInstallResult.restartRequired],
    );
    final controller = AppUpdateController(
      repository: repository,
      platform: platform,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    await controller.install();

    expect(repository.downloadCalls, 0);
    expect(controller.phase, AppUpdatePhase.restartRequired);
    expect(controller.message, contains('Quit and reopen'));
  });

  test(
    'reports an up-to-date installation without offering a package',
    () async {
      final controller = AppUpdateController(
        repository: _FakeUpdateRepository(),
        platform: _FakeUpdatePlatform(),
      );
      addTearDown(controller.dispose);

      await controller.initialize();

      expect(controller.phase, AppUpdatePhase.upToDate);
      expect(controller.release, isNull);
    },
  );
}

AppRelease _release() {
  final version = Version.parse('1.0.1');
  return AppRelease(
    version: version,
    releaseUri: Uri.parse(
      'https://github.com/dmytro-yevs/copypaste/releases/tag/v$version',
    ),
    prerelease: false,
    asset: AppReleaseAsset(
      name: 'CopyPaste-v$version-android.apk',
      downloadUri: Uri.parse(
        'https://github.com/dmytro-yevs/copypaste/releases/download/v$version/CopyPaste-v$version-android.apk',
      ),
      sha256: 'a' * 64,
      sizeBytes: 1024,
      signatureUri: Uri.parse(
        'https://github.com/dmytro-yevs/copypaste/releases/download/v$version/CopyPaste-v$version-android.apk.sig',
      ),
      signatureSha256: 'b' * 64,
      signatureSizeBytes: 512,
    ),
  );
}

class _FakeUpdateRepository implements AppUpdateRepository {
  _FakeUpdateRepository({this.release});

  final AppRelease? release;
  AppUpdateTarget? requestedTarget;
  int downloadCalls = 0;

  @override
  Future<DownloadedAppUpdate> download(
    AppRelease release, {
    required void Function(double progress) onProgress,
  }) async {
    downloadCalls += 1;
    onProgress(1);
    return DownloadedAppUpdate(
      path: '/tmp/${release.asset!.name}',
      asset: release.asset!,
    );
  }

  @override
  Future<AppRelease?> findUpdate({
    required Version currentVersion,
    required AppUpdateTarget target,
  }) async {
    requestedTarget = target;
    return release;
  }

  @override
  void dispose() {}
}

class _FakeUpdatePlatform implements AppUpdatePlatform {
  _FakeUpdatePlatform({
    this.target = AppUpdateTarget.android,
    List<AppUpdateInstallResult>? installResults,
  }) : _installResults = installResults ?? [AppUpdateInstallResult.started];

  @override
  final AppUpdateTarget target;
  final List<AppUpdateInstallResult> _installResults;
  int installCalls = 0;

  @override
  Future<AppUpdateAvailability> availability() async =>
      const AppUpdateAvailability.available();

  @override
  Future<String> currentVersion() async => '1.0.0';

  @override
  Future<AppUpdateInstallResult> install({
    required AppRelease release,
    DownloadedAppUpdate? package,
  }) async {
    final index = installCalls.clamp(0, _installResults.length - 1);
    installCalls += 1;
    return _installResults[index];
  }

  @override
  Future<void> openReleasePage(Uri uri) async {}
}
