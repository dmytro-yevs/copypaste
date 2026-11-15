import 'dart:async';

import 'package:copypaste_flutter/app/navigation/app_destination.dart';
import 'package:copypaste_flutter/app/navigation/app_navigation_controller.dart';
import 'package:copypaste_flutter/app/navigation/settings_presentation_lifecycle.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:flutter/foundation.dart';
import 'package:copypaste_flutter/features/settings/models/sync_status.dart';
import 'package:copypaste_flutter/features/settings/repository/settings_repository.dart';
import 'package:copypaste_flutter/platform/android/screenshot_capture.dart';
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

class _PushCaptureRepository extends _SlowCaptureRepository
    implements CaptureEventsRepository {
  final changes = StreamController<CaptureSettingsState?>.broadcast();
  @override
  Stream<CaptureSettingsState?> captureEvents() => changes.stream;
  @override
  Future<void> dispose() async {
    await changes.close();
    await super.dispose();
  }
}

class _PendingToggleRepository extends _PushCaptureRepository {
  final toggle = Completer<CaptureSettingsState>();
  @override
  Future<CaptureSettingsState> setCapturePaused(bool paused) => toggle.future;
}

class _AndroidScreenshotCapture implements ScreenshotCapture {
  @override
  bool get supported => true;
  @override
  Future<ScreenshotCaptureStatus> status() async =>
      const ScreenshotCaptureStatus();
  @override
  Future<ScreenshotCaptureStatus> setEnabled(bool enabled) => status();
  @override
  Future<ScreenshotCaptureStatus> requestPermission() => status();
}

class _AndroidScreenshotEvents extends _AndroidScreenshotCapture
    implements ScreenshotCaptureEvents {
  final events = StreamController<ScreenshotCaptureStatus>.broadcast();
  @override
  Stream<ScreenshotCaptureStatus> get changes => events.stream;
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
  testWidgets('initial History and visibility control settings polling', (
    tester,
  ) async {
    final repository = _SlowCaptureRepository();
    final navigation = AppNavigationController();
    final visible = ValueNotifier(true);
    SettingsController? controller;
    final lifecycle = SettingsPresentationLifecycle(
      navigation: navigation,
      visible: visible,
      setActive: (active) => controller?.setPresentationActive(active),
    );
    controller = _controller(repository);
    lifecycle.refresh();
    await controller.initialize();
    await tester.pump(const Duration(seconds: 5));
    expect(repository.reads, 1);
    repository.capture = const CaptureSettingsState(
      running: false,
      paused: true,
      epoch: 1,
    );
    navigation.selectDestination(AppDestination.settings);
    await tester.pump();
    expect(repository.reads, 2);
    expect(controller.capture?.paused, isTrue);
    visible.value = false;
    await tester.pump(const Duration(seconds: 5));
    expect(repository.reads, 2);
    visible.value = true;
    await tester.pump();
    expect(repository.reads, 3);
    lifecycle.dispose();
    controller.dispose();
    navigation.dispose();
    visible.dispose();
  });

  testWidgets('capture push stops polling and ignores stale revisions', (
    tester,
  ) async {
    final repository = _PushCaptureRepository();
    final controller = _controller(repository);
    await controller.initialize();
    repository.changes.add(null);
    repository.changes.add(
      const CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 0,
        captureWatch: true,
        captureRevision: 1,
        syncStatus: SyncStatus(revision: 7),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(seconds: 10));
    expect(repository.reads, 2);
    repository.changes.add(
      const CaptureSettingsState(
        running: false,
        paused: true,
        epoch: 1,
        captureWatch: true,
        captureRevision: 2,
      ),
    );
    repository.changes.add(
      const CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 0,
        captureWatch: true,
        captureRevision: 1,
      ),
    );
    await tester.pump();
    expect(controller.capture?.paused, isTrue);
    expect(controller.syncStatus.revision, 7);
    repository.changes.add(null);
    repository.capture = const CaptureSettingsState(
      running: true,
      paused: false,
      epoch: 0,
    );
    await tester.pump();
    await tester.pump(const Duration(seconds: 1));
    expect(repository.reads, 4);
    expect(controller.syncStatus.revision, 0);
    controller.dispose();
  });

  testWidgets('a stale failed read does not erase a newer push', (
    tester,
  ) async {
    final repository = _PushCaptureRepository();
    final controller = _controller(repository);
    await controller.initialize();
    final pending = Completer<CaptureSettingsState>();
    repository.pending = pending;
    await tester.pump(const Duration(seconds: 1));
    repository.changes.add(
      const CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 0,
        captureWatch: true,
        captureRevision: 1,
        syncStatus: SyncStatus(revision: 5, phase: SyncPhase.synced),
      ),
    );
    await tester.pump();
    pending.completeError(StateError('stale failure'));
    await tester.pump();
    expect(controller.syncStatus.revision, 5);
    expect(controller.syncStatus.phase, SyncPhase.synced);
    controller.dispose();
  });

  testWidgets('stale toggle response cannot overwrite newer capture push', (
    tester,
  ) async {
    final repository = _PendingToggleRepository();
    final controller = _controller(repository);
    await controller.initialize();
    final toggle = controller.toggleCapture();
    repository.changes.add(
      const CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 2,
        captureWatch: true,
        captureRevision: 2,
      ),
    );
    await tester.pump();
    final read = Completer<CaptureSettingsState>();
    repository.pending = read;
    repository.toggle.complete(
      const CaptureSettingsState(
        running: false,
        paused: true,
        epoch: 1,
        captureWatch: true,
        captureRevision: 1,
      ),
    );
    await tester.pump();
    expect(controller.capture?.paused, isFalse);
    read.complete(
      const CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 2,
        captureWatch: true,
        captureRevision: 2,
      ),
    );
    await tester.pump();
    expect(await toggle, isTrue);
    controller.dispose();
  });

  testWidgets('Android screenshot status retains polling', (tester) async {
    final repository = _PushCaptureRepository();
    final controller = SettingsController(
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      screenshotProtection: FakeScreenshotProtection(),
      screenshotCapture: _AndroidScreenshotCapture(),
      captureRefreshInterval: const Duration(seconds: 1),
    );
    await controller.initialize();
    repository.changes.add(null);
    await tester.pump();
    repository.changes.add(
      const CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 0,
        captureWatch: true,
        captureRevision: 1,
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(seconds: 1));
    expect(repository.reads, 3);
    controller.dispose();
  });

  testWidgets('Android screenshot events stop polling and recover on error', (
    tester,
  ) async {
    final repository = _PushCaptureRepository();
    final screenshots = _AndroidScreenshotEvents();
    final controller = SettingsController(
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      screenshotProtection: FakeScreenshotProtection(),
      screenshotCapture: screenshots,
      captureRefreshInterval: const Duration(seconds: 1),
    );
    await controller.initialize();
    expect(screenshots.events.hasListener, isFalse);
    repository.changes.add(null);
    await tester.pump();
    repository.changes.add(
      const CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 0,
        captureWatch: true,
        captureRevision: 1,
      ),
    );
    await tester.pump();
    expect(screenshots.events.hasListener, isTrue);
    screenshots.events.add(
      const ScreenshotCaptureStatus(mediaGranted: true, running: true),
    );
    await tester.pump();
    await tester.pump(const Duration(seconds: 5));
    expect(repository.reads, 2);
    expect(controller.screenshotCapture.running, isTrue);
    controller.setPresentationActive(false);
    await tester.pump();
    expect(screenshots.events.hasListener, isFalse);
    controller.setPresentationActive(true);
    await tester.pump();
    expect(screenshots.events.hasListener, isTrue);
    screenshots.events.add(
      const ScreenshotCaptureStatus(mediaGranted: true, running: true),
    );
    await tester.pump();
    screenshots.events.addError(StateError('channel unavailable'));
    await tester.pump();
    await tester.pump(const Duration(seconds: 1));
    expect(repository.reads, 4);
    controller.dispose();
    await screenshots.events.close();
  });

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
