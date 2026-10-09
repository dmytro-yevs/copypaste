import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
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

  test('native failure retains the last confirmed switch state', () async {
    final port = _Capture();
    final settings = controller(port);
    addTearDown(settings.dispose);
    await settings.initialize();
    port.fail = true;
    expect(await settings.setScreenshotCaptureEnabled(false), isFalse);
    expect(settings.screenshotCapture.enabled, isTrue);
    expect(settings.errorMessage, isNotNull);
  });

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
    if (fail) throw PlatformException(code: 'capture_drain_failed');
    current = ScreenshotCaptureStatus(
      enabled: enabled,
      mediaGranted: current.mediaGranted,
      notificationGranted: current.notificationGranted,
    );
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
