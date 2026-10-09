import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:copypaste_flutter/platform/security/screenshot_protection.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  test(
    'both privacy gates persist independently through a new controller',
    () async {
      final repository = FakeSettingsRepository();
      SettingsController create() => SettingsController(
        repository: repository,
        filePicker: FakeSettingsFilePicker(),
        notifications: FakeCaptureNotificationPort(),
        screenshotProtection: _Protection(),
        captureRefreshInterval: Duration.zero,
      );
      final first = create();
      await first.initialize();
      expect(first.settings!.skipSecret, isTrue);
      expect(first.settings!.skipTransient, isTrue);
      expect(await first.setSkipSecret(false), isTrue);
      expect(first.settings!.skipSecret, isFalse);
      expect(first.settings!.skipTransient, isTrue);
      expect(await first.setSkipTransient(false), isTrue);
      first.dispose();
      final restored = create();
      addTearDown(restored.dispose);
      await restored.initialize();
      expect(restored.settings!.skipSecret, isFalse);
      expect(restored.settings!.skipTransient, isFalse);
    },
  );

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
      'Privacy switch controls the native policy on ${platform.name}',
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
          final privacy = find.byKey(
            const ValueKey<String>('mobile-settings-section-privacy'),
          );
          await tester.tap(privacy);
          await tester.pumpAndSettle();
        } else {
          await tester.tap(
            find.descendant(
              of: find.byKey(
                const ValueKey<String>('settings-section-privacy'),
              ),
              matching: find.text('Privacy'),
            ),
          );
        }
        await tester.pump();
        await tester.pump();
        expect(find.text('Block screenshots'), findsOneWidget);
        expect(
          tester
              .widget<Switch>(find.byKey(const ValueKey('skip-secret-switch')))
              .value,
          isTrue,
        );
        expect(
          tester
              .widget<Switch>(
                find.byKey(const ValueKey('skip-transient-switch')),
              )
              .value,
          isTrue,
        );
        expect(
          tester
              .widget<Switch>(
                find.byKey(const ValueKey('block-screenshots-switch')),
              )
              .value,
          isFalse,
        );
        if (platform == TargetPlatform.android) {
          await tester.pump(const Duration(milliseconds: 500));
          expect(
            find.byKey(
              const ValueKey<String>('mobile-settings-section-privacy'),
            ),
            findsNothing,
          );
        }
        expect(
          tester
              .widget<Switch>(
                find.byKey(const ValueKey('block-screenshots-switch')),
              )
              .enabled,
          isTrue,
        );
        expect(
          tester
              .widget<Switch>(
                find.byKey(const ValueKey('block-screenshots-switch')),
              )
              .onChanged,
          isNotNull,
        );
        await tester.ensureVisible(
          find.byKey(const ValueKey('block-screenshots-switch')),
        );
        await tester.pump();
        expect(
          find.byKey(const ValueKey('block-screenshots-switch')).hitTestable(),
          findsOneWidget,
        );
        await tester.tap(
          find.byKey(const ValueKey('block-screenshots-switch')),
        );
        await tester.pump();
        await tester.pump();
        expect(protection.value, isTrue);
        expect(
          tester
              .widget<Switch>(
                find.byKey(const ValueKey('block-screenshots-switch')),
              )
              .value,
          isTrue,
        );
        await tester.tap(
          find.byKey(const ValueKey('block-screenshots-switch')),
        );
        await tester.pump();
        await tester.pump();
        expect(protection.value, isFalse);
        expect(
          tester
              .widget<Switch>(
                find.byKey(const ValueKey('block-screenshots-switch')),
              )
              .value,
          isFalse,
        );
      },
      variant: TargetPlatformVariant.only(platform),
    );
  }

  testWidgets(
    'Linux explains unavailable screenshot protection and disables its control',
    (tester) async {
      final protection = _Protection()..supported = false;
      final settings = controller(protection);
      addTearDown(settings.dispose);
      await settings.initialize();
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(child: SettingsScreen(controller: settings)),
        ),
      );
      await tester.pump();
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey<String>('settings-section-privacy')),
          matching: find.text('Privacy'),
        ),
      );
      await tester.pump();
      final control = tester.widget<Switch>(
        find.byKey(const ValueKey('block-screenshots-switch')),
      );
      expect(control.enabled, isFalse);
      expect(control.onChanged, isNull);
      expect(control.value, isFalse);
      expect(
        find.text('Screenshot blocking is unavailable on this platform.'),
        findsOneWidget,
      );
      expect(await settings.setBlockScreenshots(true), isFalse);
      expect(protection.value, isFalse);
      expect(settings.settings!.skipSecret, isTrue);
      expect(settings.settings!.skipTransient, isTrue);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets(
    'Linux refuses screenshot blocking without invoking a native policy',
    (tester) async {
      const channel = MethodChannel('test/linux-security');
      var calls = 0;
      final messenger = tester.binding.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(channel, (_) async {
        calls++;
        return true;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
      final port = MethodChannelScreenshotProtection(channel: channel);
      expect(port.supported, isFalse);
      expect(await port.blocked(), isFalse);
      await expectLater(
        port.setBlocked(true),
        throwsA(isA<PlatformException>()),
      );
      expect(calls, 0);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

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
  @override
  bool supported = true;
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
