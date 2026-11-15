import 'dart:async';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/repository/settings_repository.dart';
import 'package:copypaste_flutter/platform/android/screenshot_capture.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  SettingsController controller(_Capture capture) => SettingsController(
    repository: FakeSettingsRepository(),
    filePicker: FakeSettingsFilePicker(),
    notifications: FakeCaptureNotificationPort(),
    screenshotProtection: FakeScreenshotProtection(),
    screenshotCapture: capture,
    captureRefreshInterval: Duration.zero,
  );

  testWidgets('stopped screenshot capture reacts to native state and resumes', (
    tester,
  ) async {
    final port = _CaptureEvents()
      ..current = const ScreenshotCaptureStatus(
        enabled: true,
        mediaGranted: true,
        notificationGranted: true,
        running: false,
      );
    final repository = _CaptureEventsRepository();
    final settings = SettingsController(
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      screenshotProtection: FakeScreenshotProtection(),
      screenshotCapture: port,
      captureRefreshInterval: Duration.zero,
    );
    addTearDown(settings.dispose);
    addTearDown(port.events.close);
    addTearDown(repository.events.close);
    await settings.initialize();
    repository.events.add(null);
    await tester.pump();
    repository.events.add(
      const CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 0,
        captureWatch: true,
        captureRevision: 1,
      ),
    );
    await tester.pump();
    expect(port.events.hasListener, isTrue);
    await tester.binding.setSurfaceSize(const Size(390, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(child: SettingsScreen(controller: settings)),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('mobile-settings-section-clipboard')),
    );
    await tester.pumpAndSettle();
    expect(find.text('Screenshot capture is not running'), findsOneWidget);
    port.events.add(
      const ScreenshotCaptureStatus(
        enabled: true,
        mediaGranted: true,
        notificationGranted: true,
        running: true,
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Screenshot capture is not running'), findsNothing);
    port.events.add(port.current);
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Resume capture'));
    await tester.tap(find.text('Resume capture'));
    await tester.pumpAndSettle();
    expect(port.current.running, isTrue);
    expect(find.text('Screenshot capture is not running'), findsNothing);
    expect(settings.screenshotCapture.enabled, isTrue);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  test('enabled default and native changes survive a new controller', () async {
    final port = _Capture();
    final first = controller(port);
    addTearDown(first.dispose);
    await first.initialize();
    expect(first.screenshotCapture.enabled, isTrue);
    expect(first.screenshotCapture.needsPermission, isTrue);
    expect(await first.setScreenshotCaptureEnabled(false), isTrue);
    final restored = controller(port);
    addTearDown(restored.dispose);
    await restored.initialize();
    expect(restored.screenshotCapture.enabled, isFalse);
    expect(await restored.setScreenshotCaptureEnabled(true), isTrue);
    expect(restored.screenshotCapture.enabled, isTrue);
  });

  test(
    'native drain failure rereads the persisted screenshot switch state',
    () async {
      final port = _Capture();
      final settings = controller(port);
      addTearDown(settings.dispose);
      await settings.initialize();
      port.fail = true;
      expect(await settings.setScreenshotCaptureEnabled(false), isFalse);
      expect(settings.screenshotCapture.enabled, isFalse);
      expect(settings.errorMessage, isNotNull);
    },
  );

  for (final (width, textScale) in [
    (390.0, 1.0),
    (1000.0, 1.0),
    (320.0, 2.0),
  ]) {
    testWidgets(
      'Clipboard screenshot controls at width $width and scale $textScale',
      (tester) async {
        await tester.binding.setSurfaceSize(Size(width, 900));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final port = _Capture();
        final settings = controller(port);
        addTearDown(settings.dispose);
        await settings.initialize();
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            darkTheme: AppTheme.dark,
            themeMode: AppTheme.mode,
            builder: AppTheme.builder,
            home: Builder(
              builder: (context) => MediaQuery(
                data: MediaQuery.of(
                  context,
                ).copyWith(textScaler: TextScaler.linear(textScale)),
                child: Scaffold(child: SettingsScreen(controller: settings)),
              ),
            ),
          ),
        );
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        if (width < 1000) {
          await tester.tap(
            find.byKey(
              const ValueKey<String>('mobile-settings-section-clipboard'),
            ),
          );
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 500));
        }
        final toggle = find.byKey(const ValueKey('save-screenshots-switch'));
        expect(tester.widget<Switch>(toggle).value, isTrue);
        final permission = find.byKey(
          const ValueKey('screenshot-capture-permission'),
        );
        expect(permission, findsOneWidget);
        await tester.ensureVisible(permission);
        await tester.tap(permission);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        expect(port.permissionRequests, 1);
        expect(permission, findsNothing);
        expect(settings.screenshotCapture.needsPermission, isFalse);
        expect(
          find.byKey(const ValueKey('screenshot-source-access')),
          findsNothing,
        );
        await tester.ensureVisible(toggle);
        await tester.tap(toggle);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        expect(port.current.enabled, isFalse);
        expect(tester.widget<Switch>(toggle).value, isFalse);
      },
    );
  }

  test('screenshot state stream maps native updates and cancels', () async {
    const channel = EventChannel('test/screenshot-state-events');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    var cancelled = false;
    messenger.setMockStreamHandler(
      channel,
      MockStreamHandler.inline(
        onListen: (_, events) => events.success({
          'enabled': false,
          'mediaGranted': true,
          'notificationGranted': false,
          'running': true,
        }),
        onCancel: (_) => cancelled = true,
      ),
    );
    addTearDown(() => messenger.setMockStreamHandler(channel, null));
    const port = MethodChannelScreenshotCapture(events: channel);
    final received = Completer<ScreenshotCaptureStatus>();
    final subscription = port.changes.listen(received.complete);
    final latest = await received.future;
    expect(latest.enabled, isFalse);
    expect(latest.mediaGranted, isTrue);
    expect(latest.notificationGranted, isFalse);
    expect(latest.running, isTrue);
    await subscription.cancel();
    expect(cancelled, isTrue);
  });

  test('typed channel preserves permission denial and native state', () async {
    const channel = MethodChannel('test/screenshot-capture');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    final calls = <MethodCall>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call);
      return {
        'enabled': call.arguments == null
            ? true
            : (call.arguments as Map)['enabled'],
        'mediaGranted': false,
        'notificationGranted': true,
        'running': false,
      };
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    const port = MethodChannelScreenshotCapture(channel: channel);
    expect((await port.status()).enabled, isTrue);
    expect((await port.requestPermission()).needsPermission, isTrue);
    expect((await port.setEnabled(false)).enabled, isFalse);
    expect(calls.last.method, 'setScreenshotCaptureEnabled');
    expect(calls.last.arguments, {'enabled': false});
  });
}

class _CaptureEventsRepository extends FakeSettingsRepository
    implements CaptureEventsRepository {
  final events = StreamController<CaptureSettingsState?>.broadcast();
  @override
  Stream<CaptureSettingsState?> captureEvents() => events.stream;
}

class _CaptureEvents extends _Capture implements ScreenshotCaptureEvents {
  final events = StreamController<ScreenshotCaptureStatus>.broadcast();
  @override
  Stream<ScreenshotCaptureStatus> get changes => events.stream;

  @override
  Future<ScreenshotCaptureStatus> setEnabled(bool enabled) async {
    current = ScreenshotCaptureStatus(
      enabled: enabled,
      mediaGranted: current.mediaGranted,
      notificationGranted: current.notificationGranted,
      running: enabled && !current.needsPermission,
    );
    return current;
  }
}

class _Capture implements ScreenshotCapture {
  ScreenshotCaptureStatus current = const ScreenshotCaptureStatus();
  int permissionRequests = 0;
  bool fail = false;
  @override
  bool get supported => true;
  @override
  Future<ScreenshotCaptureStatus> status() async => current;
  @override
  Future<ScreenshotCaptureStatus> setEnabled(bool enabled) async {
    current = ScreenshotCaptureStatus(
      enabled: enabled,
      mediaGranted: current.mediaGranted,
      notificationGranted: current.notificationGranted,
    );
    if (fail) throw PlatformException(code: 'capture_drain_failed');
    return current;
  }

  @override
  Future<ScreenshotCaptureStatus> requestPermission() async {
    permissionRequests++;
    current = ScreenshotCaptureStatus(
      enabled: current.enabled,
      mediaGranted: true,
      notificationGranted: true,
      running: true,
    );
    return current;
  }
}
