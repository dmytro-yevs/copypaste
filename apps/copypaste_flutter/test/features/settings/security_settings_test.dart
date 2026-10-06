import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:copypaste_flutter/platform/security/screenshot_protection.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  SettingsController controller(_Protection protection) => SettingsController(
    repository: FakeSettingsRepository(),
    filePicker: FakeSettingsFilePicker(),
    notifications: FakeCaptureNotificationPort(),
    screenshotProtection: protection,
    captureRefreshInterval: Duration.zero,
  );

  test(
    'default is off and the native policy survives a new controller',
    () async {
      final protection = _Protection();
      final first = controller(protection);
      addTearDown(first.dispose);
      await first.initialize();
      expect(first.blockScreenshots, isFalse);
      expect(await first.setBlockScreenshots(true), isTrue);
      expect(first.blockScreenshots, isTrue);
      final second = controller(protection);
      addTearDown(second.dispose);
      await second.initialize();
      expect(second.blockScreenshots, isTrue);
      expect(await second.setBlockScreenshots(false), isTrue);
      expect(protection.value, isFalse);
    },
  );

  test('failed native changes retain the confirmed setting', () async {
    final protection = _Protection()..fail = true;
    final settings = controller(protection);
    addTearDown(settings.dispose);
    await settings.initialize();
    expect(await settings.setBlockScreenshots(true), isFalse);
    expect(settings.blockScreenshots, isFalse);
    expect(settings.errorMessage, isNotNull);
    expect(settings.busy, isFalse);
  });

  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.android,
    TargetPlatform.windows,
  ]) {
    testWidgets(
      'Security switch controls the native policy on ${platform.name}',
      (tester) async {
        if (platform == TargetPlatform.android) {
          addTearDown(() => tester.binding.setSurfaceSize(null));
          await tester.binding.setSurfaceSize(const Size(390, 720));
        }
        final protection = _Protection();
        final settings = controller(protection);
        addTearDown(settings.dispose);
        await settings.initialize();
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            darkTheme: AppTheme.dark,
            themeMode: AppTheme.mode,
            builder: AppTheme.builder,
            home: Scaffold(child: SettingsScreen(controller: settings)),
          ),
        );
        await tester.pump();
        await tester.pump();
        if (platform == TargetPlatform.android) {
          await tester.tap(
            find.descendant(
              of: find.byKey(
                const ValueKey<String>('settings-mobile-section-select'),
              ),
              matching: find.text('Capture'),
            ),
          );
          await tester.pump(const Duration(milliseconds: 500));
          await tester.ensureVisible(
            find.byKey(
              const ValueKey<String>('mobile-settings-section-security'),
            ),
          );
          await tester.pump();
          await tester.tap(
            find.descendant(
              of: find.byKey(
                const ValueKey<String>('mobile-settings-section-security'),
              ),
              matching: find.text('Security'),
            ),
          );
          await tester.pump(const Duration(milliseconds: 500));
        } else {
          await tester.tap(
            find.descendant(
              of: find.byKey(
                const ValueKey<String>('settings-section-security'),
              ),
              matching: find.text('Security'),
            ),
          );
        }
        await tester.pump();
        await tester.pump();
        expect(find.text('Block screenshots'), findsOneWidget);
        expect(tester.widget<Switch>(find.byType(Switch)).value, isFalse);
        if (platform == TargetPlatform.android) {
          await tester.pump(const Duration(milliseconds: 500));
          expect(
            find.byKey(
              const ValueKey<String>('mobile-settings-section-security'),
            ),
            findsNothing,
          );
        }
        expect(tester.widget<Switch>(find.byType(Switch)).enabled, isTrue);
        expect(tester.widget<Switch>(find.byType(Switch)).onChanged, isNotNull);
        await tester.tap(find.byType(Switch));
        await tester.pump();
        await tester.pump();
        expect(protection.value, isTrue);
        expect(tester.widget<Switch>(find.byType(Switch)).value, isTrue);
        await tester.tap(find.byType(Switch));
        await tester.pump();
        await tester.pump();
        expect(protection.value, isFalse);
        expect(tester.widget<Switch>(find.byType(Switch)).value, isFalse);
      },
      variant: TargetPlatformVariant.only(platform),
    );
  }

  test('the channel sends the policy and rejects native failure', () async {
    const channel = MethodChannel('test/security');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    final port = MethodChannelScreenshotProtection(channel: channel);
    final calls = <MethodCall>[];
    var applied = true;
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call);
      return call.method == 'getBlockScreenshots' ? false : applied;
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    expect(await port.blocked(), isFalse);
    await port.setBlocked(true);
    expect(calls.last.method, 'setBlockScreenshots');
    expect(calls.last.arguments, {'enabled': true});
    applied = false;
    await expectLater(
      port.setBlocked(false),
      throwsA(isA<PlatformException>()),
    );
  });
}

class _Protection implements ScreenshotProtection {
  bool value = false;
  bool fail = false;
  @override
  Future<bool> blocked() async => value;
  @override
  Future<void> setBlocked(bool blocked) async {
    if (fail) throw PlatformException(code: 'failed');
    value = blocked;
  }
}
