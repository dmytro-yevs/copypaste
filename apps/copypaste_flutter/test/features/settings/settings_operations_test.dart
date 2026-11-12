import 'dart:async';

import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/repository/settings_repository.dart';
import 'package:flutter_test/flutter_test.dart';

import 'settings_test_support.dart';

void main() {
  Future<(SettingsController, _GatedRepository)> start({
    CaptureSettingsState? capture,
  }) async {
    final repository = _GatedRepository();
    if (capture != null) repository.capture = capture;
    final controller = SettingsController(
      screenshotProtection: FakeScreenshotProtection(),
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    return (controller, repository);
  }

  test(
    'a second change starts while one is pending and both are kept',
    () async {
      final (controller, repository) = await start();
      repository.hold = true;

      final sync = controller.setSyncEnabled(false);
      final sound = controller.setSoundOnCopy(true);

      expect(controller.isPending(SettingsOperation.syncEnabled), isTrue);
      expect(controller.isPending(SettingsOperation.soundOnCopy), isTrue);
      expect(controller.isPending(SettingsOperation.lanVisibility), isFalse);
      expect(controller.canStart(SettingsOperation.lanVisibility), isTrue);
      expect(controller.canStart(SettingsOperation.syncEnabled), isFalse);
      // Whole records come back, so the writes run one after the other.
      await pumpEventQueue();
      expect(repository.updates, 1);

      repository.release();
      await pumpEventQueue();
      expect(repository.updates, 2);
      repository.release();
      expect(await sync, isTrue);
      expect(await sound, isTrue);
      expect(controller.settings?.syncEnabled, isFalse);
      expect(controller.settings?.soundOnCopy, isTrue);
      expect(controller.busy, isFalse);
    },
  );

  test(
    'a failure belongs to its own change and stays until the next one',
    () async {
      final (controller, repository) = await start();
      repository.hold = true;

      final sync = controller.setSyncEnabled(false);
      final sound = controller.setSoundOnCopy(true);
      repository.failNext = true;
      await pumpEventQueue();
      repository.release();
      expect(await sync, isFalse);
      await pumpEventQueue();
      repository.release();
      expect(await sound, isTrue);

      const failure = 'The requested change could not be completed.';
      expect(controller.errorFor(SettingsOperation.syncEnabled), failure);
      expect(controller.errorFor(SettingsOperation.soundOnCopy), isNull);
      // The later success does not hide the earlier failure.
      expect(controller.errorMessage, failure);

      repository.hold = false;
      // The next change hides the shared banner; the failure stays with its
      // own change until that change runs again.
      expect(await controller.setLanVisibility(false), isTrue);
      expect(controller.errorFor(SettingsOperation.syncEnabled), failure);
      expect(controller.errorMessage, isNull);
      expect(await controller.setSyncEnabled(false), isTrue);
      expect(controller.errorFor(SettingsOperation.syncEnabled), isNull);
      expect(controller.errorMessage, isNull);
    },
  );

  test('a backup failure survives a change to another setting', () async {
    final (controller, repository) = await start();
    repository.failHistory = true;
    expect(await controller.createBackup(), isNull);
    final failure = controller.errorFor(SettingsOperation.backup);
    expect(failure, isNotNull);

    expect(await controller.setSkipSecret(true), isTrue);
    expect(controller.errorFor(SettingsOperation.backup), failure);
  });

  test('export and backup present no file once disposed', () async {
    final repository = _GatedRepository()..holdHistory = true;
    final picker = FakeSettingsFilePicker();
    for (final transfer in [
      (SettingsController controller) => controller.exportTextHistory(),
      (SettingsController controller) => controller.createBackup(),
    ]) {
      final controller = SettingsController(
        screenshotProtection: FakeScreenshotProtection(),
        repository: repository,
        filePicker: picker,
        notifications: FakeCaptureNotificationPort(),
        captureRefreshInterval: Duration.zero,
      );
      await controller.initialize();
      final running = transfer(controller);
      await pumpEventQueue();
      controller.dispose();
      repository.releaseHistory();
      await running;
    }
    expect(picker.presentedPaths, isEmpty);
  });

  test('dismissing hides the failure and the current health notice', () async {
    final (controller, repository) = await start(
      capture: const CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 0,
        settingsHealth: RuntimeSettingsHealth(
          recordUnreadable: true,
          unreadableFields: [],
        ),
      ),
    );
    repository.failNext = true;
    expect(await controller.setSyncEnabled(false), isFalse);
    expect(controller.errorMessage, isNot(contains('saved settings')));

    controller.clearMessages();
    expect(controller.errorMessage, isNull);

    // A notice reported again after it cleared is shown again.
    expect(await controller.setSyncEnabled(false), isTrue);
    expect(controller.errorMessage, isNull);
    repository.capture = const CaptureSettingsState(
      running: true,
      paused: false,
      epoch: 0,
      captureRevision: 1,
      settingsHealth: RuntimeSettingsHealth(
        recordUnreadable: true,
        unreadableFields: [],
      ),
    );
    controller.setPresentationActive(false);
    controller.setPresentationActive(true);
    await pumpEventQueue();
    expect(controller.errorMessage, contains('saved settings'));
  });

  test(
    'restore excludes every change; export and backup exclude each other',
    () async {
      final (controller, repository) = await start();
      repository.holdHistory = true;

      final export = controller.exportTextHistory();
      await pumpEventQueue();
      expect(controller.isPending(SettingsOperation.exportText), isTrue);
      expect(controller.canStart(SettingsOperation.backup), isFalse);
      expect(controller.canStart(SettingsOperation.restore), isFalse);
      expect(controller.canStart(SettingsOperation.retention), isTrue);
      repository.releaseHistory();
      expect(await export, isNotNull);

      final restore = controller.restoreBackup();
      await pumpEventQueue();
      expect(controller.canStart(SettingsOperation.syncEnabled), isFalse);
      expect(controller.canStart(SettingsOperation.capture), isFalse);
      expect(await controller.toggleCapture(), isFalse);
      repository.releaseHistory();
      expect(await restore, isTrue);
      expect(controller.canStart(SettingsOperation.capture), isTrue);
    },
  );
}

class _GatedRepository extends FakeSettingsRepository {
  bool hold = false;
  bool failNext = false;
  bool holdHistory = false;
  bool failHistory = false;
  int updates = 0;
  final _gates = <Completer<void>>[];
  final _historyGates = <Completer<void>>[];

  void release() => _gates.removeAt(0).complete();
  void releaseHistory() => _historyGates.removeAt(0).complete();

  @override
  Future<RuntimeSettings> updateSettings(RuntimeSettingsChange change) async {
    updates += 1;
    if (hold) {
      final gate = Completer<void>();
      _gates.add(gate);
      await gate.future;
    }
    if (failNext) {
      failNext = false;
      throw StateError('write failed');
    }
    return super.updateSettings(change);
  }

  @override
  Future<TextExportResult> exportTextHistory(
    String path, {
    DataTransferProgressCallback? onProgress,
  }) async {
    await _historyGate();
    return super.exportTextHistory(path, onProgress: onProgress);
  }

  @override
  Future<BackupResult> backupHistory(
    String path, {
    DataTransferProgressCallback? onProgress,
  }) async {
    await _historyGate();
    return super.backupHistory(path, onProgress: onProgress);
  }

  @override
  Future<void> restoreHistory(String path) async {
    await _historyGate();
    return super.restoreHistory(path);
  }

  Future<void> _historyGate() async {
    if (failHistory) throw StateError('history unavailable');
    if (!holdHistory) return;
    final gate = Completer<void>();
    _historyGates.add(gate);
    await gate.future;
  }
}
