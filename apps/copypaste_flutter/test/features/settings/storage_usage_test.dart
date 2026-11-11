import 'dart:async';

import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/repository/runtime_settings_repository.dart';
import 'package:copypaste_flutter/features/settings/repository/settings_repository.dart';
import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:flutter_test/flutter_test.dart';

import 'settings_test_support.dart';

const _gib = 1024 * 1024 * 1024;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late _Repository repository;
  late SettingsController controller;
  setUp(() async {
    repository = _Repository()..report(usedBytes: 100);
    controller = SettingsController(
      repository: repository,
      notifications: FakeCaptureNotificationPort(),
      screenshotProtection: FakeScreenshotProtection(),
      filePicker: FakeSettingsFilePicker(),
      captureRefreshInterval: Duration.zero,
    );
    await controller.initialize();
  });
  tearDown(() => controller.dispose());

  test('a settings reply maps usage beside the quota it was read with', () {
    runtime.RuntimeSettingsData reply(runtime.RuntimeStorageUsage? usage) =>
        runtime.RuntimeSettingsData(
          maxFileSizeBytes: BigInt.from(64 * 1024 * 1024),
          skipSecret: true,
          skipTransient: true,
          retentionDays: 0,
          storageQuotaBytes: BigInt.from(5 * _gib),
          excludedAppIds: const [],
          lanVisibility: true,
          syncEnabled: true,
          instantClipboard: true,
          notifyOnCopy: false,
          notificationPreview: true,
          soundOnCopy: false,
          storageUsage: usage,
        );

    final mapped = RuntimeSettingsRepository.mapSettings(
      reply(
        runtime.RuntimeStorageUsage(
          usedBytes: BigInt.from(2 * _gib),
          quotaBytes: BigInt.from(5 * _gib),
          oldestUnpinnedAtMs: 1700000000000,
        ),
      ),
    );
    expect(
      mapped.storageUsage,
      StorageUsage(
        usedBytes: 2 * _gib,
        quotaBytes: 5 * _gib,
        oldestUnpinnedAt: DateTime.fromMillisecondsSinceEpoch(1700000000000),
      ),
    );
    expect(
      RuntimeSettingsRepository.mapSettings(reply(null)).storageUsage,
      isNull,
      reason: 'An unreported readout must not become zero bytes',
    );
  });

  test('history changes re-read usage only while it is displayed', () async {
    expect(controller.storageUsage.value!.usedBytes, 100);
    final loaded = repository.reads;

    repository.report(usedBytes: 200);
    repository.changes.add(null);
    await pumpEventQueue();
    expect(repository.reads, loaded, reason: 'Nobody displays the readout');

    var notified = 0;
    void listener() => notified++;
    controller.storageUsage.addListener(listener);
    await pumpEventQueue();
    expect(controller.storageUsage.value!.usedBytes, 200);
    expect(repository.reads, loaded + 1);
    expect(notified, 1);

    final delayed = Completer<RuntimeSettings>();
    repository.nextRead = delayed;
    repository.report(usedBytes: 300);
    for (var burst = 0; burst < 3; burst++) {
      repository.changes.add(null);
    }
    await pumpEventQueue();
    delayed.complete(repository.currentSettings);
    await pumpEventQueue();
    expect(
      repository.reads,
      loaded + 3,
      reason: 'A burst is one trailing read',
    );
    expect(controller.storageUsage.value!.usedBytes, 300);

    final unchanged = notified;
    repository.changes.add(null);
    await pumpEventQueue();
    expect(notified, unchanged, reason: 'An equal readout is not a change');

    controller.storageUsage.removeListener(listener);
    final hidden = repository.reads;
    repository.changes.add(null);
    await pumpEventQueue();
    expect(repository.reads, hidden);
  });

  test(
    'a settings change reports the usage the runtime answered with',
    () async {
      repository.report(usedBytes: 40, quotaBytes: 50 * 1024 * 1024);
      expect(await controller.setStorageQuotaBytes(50 * 1024 * 1024), isTrue);
      expect(
        controller.storageUsage.value,
        const StorageUsage(usedBytes: 40, quotaBytes: 50 * 1024 * 1024),
      );
    },
  );
}

class _Repository extends FakeSettingsRepository
    implements HistoryChangesRepository {
  final changes = StreamController<void>.broadcast();
  int reads = 0;
  Completer<RuntimeSettings>? nextRead;

  void report({required int usedBytes, int quotaBytes = 10 * _gib}) {
    currentSettings = RuntimeSettings(
      retentionDays: 0,
      storageQuotaBytes: quotaBytes,
      excludedAppIds: const [],
      lanVisibility: true,
      syncEnabled: true,
      notifyOnCopy: false,
      soundOnCopy: false,
      storageUsage: StorageUsage(usedBytes: usedBytes, quotaBytes: quotaBytes),
    );
  }

  @override
  Future<RuntimeSettings> settings() async {
    reads++;
    final pending = nextRead;
    nextRead = null;
    return pending == null ? currentSettings : pending.future;
  }

  @override
  Stream<void> historyChanges() => changes.stream;

  @override
  Future<void> dispose() async {
    await changes.close();
    await super.dispose();
  }
}
