import 'dart:async';

import 'package:copypaste_flutter/features/update/update.dart';
import 'package:copypaste_flutter/platform/update/app_update_platform.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart';
import 'package:pub_semver/pub_semver.dart';

void main() {
  test('waits for Android confirmation and reports cancellation', () async {
    final completion = Completer<AppUpdateInstallResult>();
    final platform = _FakeUpdatePlatform(installCompletion: completion);
    final controller = AppUpdateController(
      repository: _FakeUpdateRepository(release: _release()),
      platform: platform,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    final installation = controller.install();
    await Future<void>.delayed(Duration.zero);
    expect(controller.phase, AppUpdatePhase.installing);
    expect(controller.busy, isTrue);
    await controller.install();
    expect(platform.installCalls, 1);

    completion.completeError(PlatformException(code: 'installation_cancelled'));
    await installation;
    expect(controller.phase, AppUpdatePhase.error);
    expect(controller.busy, isFalse);
    expect(controller.message, 'The update installation was cancelled.');
  });

  test('reads the installed version after Android confirms success', () async {
    final completion = Completer<AppUpdateInstallResult>();
    final platform = _FakeUpdatePlatform(installCompletion: completion);
    final controller = AppUpdateController(
      repository: _FakeUpdateRepository(release: _release()),
      platform: platform,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    final installation = controller.install();
    await Future<void>.delayed(Duration.zero);
    platform.version = '1.0.1';
    completion.complete(AppUpdateInstallResult.installed);
    await installation;
    expect(controller.currentVersion, Version.parse('1.0.1'));
    expect(controller.phase, AppUpdatePhase.upToDate);
  });

  test('restores an active Android session and reports its failure', () async {
    final completion = Completer<AppUpdateInstallResult?>();
    final controller = AppUpdateController(
      repository: _FakeUpdateRepository(release: _release()),
      platform: _FakeUpdatePlatform(restoreCompletion: completion),
    );
    addTearDown(controller.dispose);
    final initialization = controller.initialize();
    await Future<void>.delayed(Duration.zero);
    expect(controller.phase, AppUpdatePhase.installing);
    completion.completeError(PlatformException(code: 'installation_storage'));
    await initialization;
    expect(controller.phase, AppUpdatePhase.error);
    expect(controller.message, 'Free up storage to install the update.');
  });

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

  test('Linux selects an update only for its installed package format', () async {
    const installation = LinuxAppUpdateInstallation(
      package: LinuxAppUpdatePackage.rpm,
      architecture: LinuxAppUpdateArchitecture.aarch64,
    );
    final repository = _FakeLinuxUpdateRepository(release: _release());
    final controller = AppUpdateController(
      repository: repository,
      platform: _FakeUpdatePlatform(
        target: AppUpdateTarget.linux,
        availabilityResult: const AppUpdateAvailability.available(
          linuxInstallation: installation,
        ),
      ),
    );
    addTearDown(controller.dispose);

    await controller.initialize();

    expect(controller.phase, AppUpdatePhase.available);
    expect(repository.requestedInstallation, installation);
    await controller.install();
    expect(controller.phase, AppUpdatePhase.installing);
    expect(controller.message, 'Continue in your system package manager.');
  });

  test('reports Linux host verification failures without claiming installation', () async {
    final completion = Completer<AppUpdateInstallResult>();
    final controller = AppUpdateController(
      repository: _FakeUpdateRepository(release: _release()),
      platform: _FakeUpdatePlatform(installCompletion: completion),
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    final installation = controller.install();
    completion.completeError(PlatformException(code: 'verification_failed'));
    await installation;

    expect(controller.phase, AppUpdatePhase.error);
    expect(
      controller.message,
      'The downloaded update failed its integrity check.',
    );
  });

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
      expect(controller.phase, AppUpdatePhase.installing);
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
    expect(
      controller.message,
      'The update is installed. Restart CopyPaste to use it.',
    );
  });

  test('restart waits for completion and ignores duplicate requests', () async {
    final completion = Completer<void>();
    var restartCalls = 0;
    final controller = AppUpdateController(
      repository: _FakeUpdateRepository(release: _release()),
      platform: _FakeUpdatePlatform(
        target: AppUpdateTarget.macos,
        installResults: [AppUpdateInstallResult.restartRequired],
      ),
      restart: () {
        restartCalls++;
        return completion.future;
      },
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.restartApplication();
    expect(restartCalls, 0);
    await controller.install();
    final restart = controller.restartApplication();
    expect(controller.busy, isTrue);
    await controller.restartApplication();
    expect(restartCalls, 1);
    completion.complete();
    await restart;
  });

  test(
    'failed restart preserves the installed update and can be retried',
    () async {
      var restartCalls = 0;
      final controller = AppUpdateController(
        repository: _FakeUpdateRepository(release: _release()),
        platform: _FakeUpdatePlatform(
          target: AppUpdateTarget.macos,
          installResults: [AppUpdateInstallResult.restartRequired],
        ),
        restart: () async {
          if (++restartCalls == 1) {
            throw PlatformException(code: 'restart_failed');
          }
        },
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.install();
      await controller.restartApplication();
      expect(controller.phase, AppUpdatePhase.restartRequired);
      expect(controller.busy, isFalse);
      expect(controller.message, 'CopyPaste could not restart. Try again.');
      await controller.restartApplication();
      expect(restartCalls, 2);
    },
  );

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
    return release != null && release!.version > currentVersion
        ? release
        : null;
  }

  @override
  void dispose() {}
}

class _FakeLinuxUpdateRepository extends _FakeUpdateRepository
    implements LinuxAppUpdateRepository {
  _FakeLinuxUpdateRepository({super.release});

  LinuxAppUpdateInstallation? requestedInstallation;

  @override
  Future<AppRelease?> findLinuxUpdate({
    required Version currentVersion,
    required LinuxAppUpdateInstallation installation,
  }) async {
    requestedInstallation = installation;
    return release != null && release!.version > currentVersion
        ? release
        : null;
  }
}

class _FakeUpdatePlatform implements AppUpdatePlatform {
  _FakeUpdatePlatform({
    this.target = AppUpdateTarget.android,
    List<AppUpdateInstallResult>? installResults,
    this.installCompletion,
    this.restoreCompletion,
    this.availabilityResult = const AppUpdateAvailability.available(),
  }) : _installResults = installResults ?? [AppUpdateInstallResult.started];

  @override
  final AppUpdateTarget target;
  final List<AppUpdateInstallResult> _installResults;
  int installCalls = 0;
  String version = '1.0.0';
  final Completer<AppUpdateInstallResult>? installCompletion;
  final Completer<AppUpdateInstallResult?>? restoreCompletion;
  final AppUpdateAvailability availabilityResult;

  @override
  Future<AppUpdateInstallResult?> restoreInstallation() async =>
      restoreCompletion == null ? null : await restoreCompletion!.future;

  @override
  Future<AppUpdateAvailability> availability() async =>
      availabilityResult;

  @override
  Future<String> currentVersion() async => version;

  @override
  Future<AppUpdateInstallResult> install({
    required AppRelease release,
    DownloadedAppUpdate? package,
  }) async {
    final index = installCalls.clamp(0, _installResults.length - 1);
    installCalls += 1;
    if (installCompletion != null) return installCompletion!.future;
    return _installResults[index];
  }

  @override
  Future<void> openReleasePage(Uri uri) async {}
}
