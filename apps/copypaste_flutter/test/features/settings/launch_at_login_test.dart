import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/launch_at_login_controller.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:copypaste_flutter/platform/startup/launch_at_login.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  for (final platform in ['macos', 'windows', 'android', 'linux']) {
    test('$platform startup reads back native registration', () async {
      final channel = MethodChannel('test/startup/$platform');
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      var enabled = false;
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'setEnabled' ||
            call.method == 'setStartAtLogin' ||
            call.method == 'setLaunchAtLogin') {
          enabled = (call.arguments as Map)['enabled'] as bool;
          return platform == 'macos'
              ? (enabled ? 'enabled' : 'not_registered')
              : true;
        }
        return switch (call.method) {
          'loginItemStatus' => enabled ? 'enabled' : 'not_registered',
          'status' => {'startAtLogin': enabled},
          'getEnabled' => enabled,
          _ => null,
        };
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
      final controller = LaunchAtLoginController(
        MethodChannelLaunchAtLoginGateway(channel: channel, platform: platform),
      );
      addTearDown(controller.dispose);
      await controller.refresh();
      expect(controller.status, LaunchAtLoginStatus.disabled);
      await controller.setEnabled(true);
      expect(controller.enabled, isTrue);
      await controller.setEnabled(false);
      expect(controller.enabled, isFalse);
      expect(controller.error, isNull);
    });
  }

  test(
    'approval, failed changes, and external settings retain actual state',
    () async {
      final gateway = _Gateway();
      final controller = LaunchAtLoginController(gateway);
      addTearDown(controller.dispose);
      await controller.refresh();
      gateway.next = LaunchAtLoginStatus.requiresApproval;
      await controller.setEnabled(true);
      expect(controller.enabled, isTrue);
      expect(controller.status, LaunchAtLoginStatus.requiresApproval);
      await controller.openSystemSettings();
      expect(gateway.settingsOpened, isTrue);
      gateway.fail = true;
      await controller.setEnabled(false);
      expect(controller.enabled, isTrue);
      expect(controller.error, isNotNull);
      gateway.fail = false;
      gateway.next = LaunchAtLoginStatus.disabled;
      await controller.refresh();
      expect(controller.enabled, isFalse);
      expect(controller.error, isNull);
    },
  );

  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.android,
  ]) {
    for (final width in [390.0, 1400.0]) {
      testWidgets('startup is searchable and editable at $width', (
        tester,
      ) async {
        await tester.binding.setSurfaceSize(Size(width, 900));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final settings = SettingsController(
          repository: FakeSettingsRepository(),
          filePicker: FakeSettingsFilePicker(),
          notifications: FakeCaptureNotificationPort(),
          screenshotProtection: FakeScreenshotProtection(),
          captureRefreshInterval: Duration.zero,
        );
        addTearDown(settings.dispose);
        await settings.initialize();
        final startup = LaunchAtLoginController(_Gateway());
        addTearDown(startup.dispose);
        await startup.refresh();
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            builder: AppTheme.builder,
            home: Scaffold(
              child: SettingsScreen(
                controller: settings,
                launchAtLogin: startup,
              ),
            ),
          ),
        );
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        await tester.enterText(
          find.byKey(const ValueKey('settings-search')),
          'startup',
        );
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        final result = find.byKey(
          ValueKey(
            width < 800
                ? 'mobile-settings-result-start-at-login'
                : 'settings-result-start-at-login',
          ),
        );
        await tester.tap(
          width < 800
              ? result
              : find
                    .descendant(of: result, matching: find.byType(Button))
                    .first,
        );
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        final toggle = find.byKey(const ValueKey('settings-start-at-login'));
        expect(tester.widget<Switch>(toggle).value, isFalse);
        await tester.tap(toggle);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        expect(startup.enabled, isTrue);
        expect(tester.widget<Switch>(toggle).value, isTrue);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      }, variant: TargetPlatformVariant({platform}));
    }
  }
}

class _Gateway implements LaunchAtLoginGateway {
  LaunchAtLoginStatus next = LaunchAtLoginStatus.disabled;
  bool fail = false;
  bool settingsOpened = false;

  @override
  Future<LaunchAtLoginStatus> read() async => next;

  @override
  Future<LaunchAtLoginStatus> setEnabled(bool enabled) async {
    if (fail) throw StateError('Registration failed');
    if (next != LaunchAtLoginStatus.requiresApproval) {
      next = enabled
          ? LaunchAtLoginStatus.enabled
          : LaunchAtLoginStatus.disabled;
    }
    return next;
  }

  @override
  Future<void> openSystemSettings() async => settingsOpened = true;
}
