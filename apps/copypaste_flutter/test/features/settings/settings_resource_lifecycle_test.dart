import 'dart:async';

import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:flutter_test/flutter_test.dart';

import 'settings_test_support.dart';

class _SlowCaptureRepository extends FakeSettingsRepository {
  int reads = 0;
  Completer<CaptureSettingsState>? pending;

  @override
  Future<CaptureSettingsState> captureState() {
    reads++;
    return pending?.future ?? Future.value(capture);
  }
}

SettingsController _controller(_SlowCaptureRepository repository) =>
    SettingsController(
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      screenshotProtection: FakeScreenshotProtection(),
      captureRefreshInterval: const Duration(seconds: 1),
    );

void main() {
  testWidgets('hidden tray keeps authoritative capture state fresh', (
    tester,
  ) async {
    final repository = _SlowCaptureRepository();
    final controller = SettingsController(
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      screenshotProtection: FakeScreenshotProtection(),
      captureRefreshInterval: const Duration(seconds: 1),
      backgroundCaptureStatusRequired: true,
    );
    await controller.initialize();
    controller.setPresentationActive(false);
    await tester.pump();
    repository.capture = const CaptureSettingsState(
      running: false,
      paused: true,
      epoch: 1,
    );
    await tester.pump(const Duration(seconds: 1));
    expect(controller.capture?.paused, isTrue);
    expect(controller.capture?.epoch, 1);
    controller.dispose();
  });

  testWidgets(
    'hidden presentation stops polling and resumes with fresh state',
    (tester) async {
      final repository = _SlowCaptureRepository();
      final controller = _controller(repository);
      await controller.initialize();
      expect(repository.reads, 1);
      controller.setPresentationActive(false);
      await tester.pump(const Duration(seconds: 10));
      expect(repository.reads, 1);
      repository.capture = const CaptureSettingsState(
        running: false,
        paused: true,
        epoch: 1,
      );
      controller.setPresentationActive(true);
      await tester.pump();
      expect(repository.reads, 2);
      expect(controller.capture?.paused, isTrue);
      controller.dispose();
    },
  );

  testWidgets('slow polling stays single flight across hide and show', (
    tester,
  ) async {
    final repository = _SlowCaptureRepository();
    final controller = _controller(repository);
    await controller.initialize();
    final pending = Completer<CaptureSettingsState>();
    repository.pending = pending;
    await tester.pump(const Duration(seconds: 1));
    await tester.pump(const Duration(seconds: 5));
    expect(repository.reads, 2);
    controller.setPresentationActive(false);
    controller.setPresentationActive(true);
    expect(repository.reads, 2);
    repository.pending = null;
    repository.capture = const CaptureSettingsState(
      running: false,
      paused: true,
      epoch: 2,
    );
    pending.complete(
      const CaptureSettingsState(running: true, paused: false, epoch: 1),
    );
    await tester.pump();
    expect(repository.reads, 3);
    expect(controller.capture?.epoch, 2);
    expect(controller.capture?.paused, isTrue);
    controller.dispose();
  });

  testWidgets('a hidden startup never creates a periodic refresh', (
    tester,
  ) async {
    final repository = _SlowCaptureRepository();
    final controller = _controller(repository)..setPresentationActive(false);
    await controller.initialize();
    await tester.pump(const Duration(seconds: 10));
    expect(repository.reads, 1);
    controller.dispose();
  });

  testWidgets('disposing an outstanding refresh does not schedule more work', (
    tester,
  ) async {
    final repository = _SlowCaptureRepository();
    final controller = _controller(repository);
    await controller.initialize();
    final pending = Completer<CaptureSettingsState>();
    repository.pending = pending;
    await tester.pump(const Duration(seconds: 1));
    controller.dispose();
    pending.completeError(StateError('closed'));
    await tester.pump(const Duration(seconds: 10));
    expect(repository.reads, 2);
  });
}
