import 'dart:async';

import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/repository/settings_repository.dart';
import 'package:flutter_test/flutter_test.dart';

import 'settings_test_support.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late _Repository repository;
  late FakeCaptureNotificationPort notifications;
  late SettingsController controller;
  setUp(() async {
    repository = _Repository();
    notifications = FakeCaptureNotificationPort();
    controller = SettingsController(
      repository: repository,
      notifications: notifications,
      screenshotProtection: FakeScreenshotProtection(),
      filePicker: FakeSettingsFilePicker(),
      captureRefreshInterval: Duration.zero,
    );
    await controller.initialize();
  });
  tearDown(() => controller.dispose());

  testWidgets(
    'fallback polling reconciles configuration only on revision changes',
    (tester) async {
      final polled = _Repository();
      final fallback = SettingsController(
        repository: polled,
        notifications: FakeCaptureNotificationPort(),
        screenshotProtection: FakeScreenshotProtection(),
        filePicker: FakeSettingsFilePicker(),
        captureRefreshInterval: const Duration(seconds: 3),
      );
      try {
        await fallback.initialize();
        await polled.updateSettings(
          const RuntimeSettingsChange(soundOnCopy: true),
        );
        polled.capture = const CaptureSettingsState(
          running: true,
          paused: false,
          epoch: 0,
          captureRevision: 1,
        );
        await tester.pump(const Duration(seconds: 3));
        await tester.pump();
        expect(fallback.capture!.captureRevision, 1);
        expect(fallback.settings!.soundOnCopy, isTrue);
        final reads = polled.reads;
        await tester.pump(const Duration(seconds: 3));
        await tester.pump();
        expect(polled.reads, reads);
      } finally {
        fallback.dispose();
        await tester.pump();
      }
    },
  );

  test('configuration events reconcile notification decisions before the next capture', () async {
    await repository.updateSettings(
      const RuntimeSettingsChange(notifyOnCopy: true),
    );
    repository.emit(1);
    repository.captures.add('clip');
    await Future<void>.delayed(Duration.zero);
    expect(controller.settings!.notifyOnCopy, isTrue);
    expect(notifications.notifications, 1);
    final reads = repository.reads;
    repository.captures.add('next-clip');
    await Future<void>.delayed(Duration.zero);
    expect(notifications.notifications, 2);
    expect(
      repository.reads,
      reads,
      reason: 'Captures must not poll configuration',
    );
    await repository.updateSettings(
      const RuntimeSettingsChange(notifyOnCopy: false),
    );
    repository.emit(2);
    repository.captures.add('silent-clip');
    await Future<void>.delayed(Duration.zero);
    expect(controller.settings!.notifyOnCopy, isFalse);
    expect(notifications.notifications, 2);
  });

  test(
    'reconnect reads configuration even when the runtime revision restarts',
    () async {
      await repository.updateSettings(
        const RuntimeSettingsChange(soundOnCopy: true),
      );
      repository.events.add(null);
      await Future<void>.delayed(Duration.zero);
      expect(controller.settings!.soundOnCopy, isTrue);
    },
  );

  test('a failed configuration refresh cannot reuse an obsolete notification policy', () async {
    await controller.setNotifyOnCopy(true);
    await repository.updateSettings(
      const RuntimeSettingsChange(notifyOnCopy: false),
    );
    final delayed = Completer<RuntimeSettings>();
    repository.nextRead = delayed;
    repository.emit(1);
    await Future<void>.delayed(Duration.zero);
    repository.captures.add('old-policy');
    delayed.completeError(StateError('configuration temporarily unavailable'));
    await Future<void>.delayed(Duration.zero);
    expect(notifications.notifications, 0);
    expect(controller.errorMessage, contains('could not be refreshed'));
    repository.emit(2);
    await Future<void>.delayed(Duration.zero);
    expect(controller.settings!.notifyOnCopy, isFalse);
    expect(controller.errorMessage, isNull);
  });

  test(
    'a pre-mutation configuration read cannot undo a newer local change',
    () async {
      final old = repository.currentSettings;
      final delayed = Completer<RuntimeSettings>();
      repository.nextRead = delayed;
      repository.emit(1);
      await Future<void>.delayed(Duration.zero);
      expect(await controller.setSoundOnCopy(true), isTrue);
      delayed.complete(old);
      await Future<void>.delayed(Duration.zero);
      expect(controller.settings!.soundOnCopy, isTrue);
      expect(repository.currentSettings.soundOnCopy, isTrue);
    },
  );
}

class _Repository extends FakeSettingsRepository
    implements CaptureEventsRepository {
  final events = StreamController<CaptureSettingsState?>.broadcast();
  int reads = 0;
  Completer<RuntimeSettings>? nextRead;
  @override
  Future<RuntimeSettings> settings() async {
    reads++;
    final pending = nextRead;
    nextRead = null;
    return pending == null ? currentSettings : pending.future;
  }

  @override
  Stream<CaptureSettingsState?> captureEvents() => events.stream;
  void emit(int revision) => events.add(
    CaptureSettingsState(
      running: true,
      paused: false,
      epoch: 0,
      captureRevision: revision,
    ),
  );
  @override
  Future<void> dispose() async {
    await events.close();
    await super.dispose();
  }
}
