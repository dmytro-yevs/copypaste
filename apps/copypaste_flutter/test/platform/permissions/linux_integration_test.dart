import 'package:copypaste_flutter/platform/permissions/linux_integration.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/linux_integration');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  late MethodChannelLinuxIntegrationPort port;

  setUp(() {
    port = MethodChannelLinuxIntegrationPort(channel: channel);
    messenger.setMockMethodCallHandler(channel, (call) async {
      return switch (call.method) {
        'status' => {
          'session': 'wayland',
          'globalShortcuts': true,
          'remoteDesktop': 'active',
          'companion': 'active',
          'clipboard': true,
          'quickPaste': true,
          'screenshotProtection': false,
          'startAtLogin': false,
          'uriRegistered': false,
        },
        'requestRemoteDesktop' => true,
        'openCompanionSetup' => true,
        'setStartAtLogin' => true,
        'registerCopypasteUri' => true,
        _ => null,
      };
    });
  });

  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  test('decodes the typed Linux integration state', () async {
    final status = await port.status();

    expect(status.session, LinuxDesktopSession.wayland);
    expect(status.globalShortcuts, isTrue);
    expect(status.remoteDesktop, LinuxRemoteDesktopState.active);
    expect(status.companion, LinuxCompanionState.active);
    expect(status.clipboard, isTrue);
    expect(status.quickPaste, isTrue);
    expect(status.screenshotProtection, isFalse);
    expect(status.startAtLogin, isFalse);
    expect(status.uriRegistered, isFalse);
  });

  test(
    'passes consent and companion setup results through unchanged',
    () async {
      expect(await port.requestRemoteDesktop(), isTrue);
      expect(await port.openCompanionSetup(), isTrue);
      expect(await port.setStartAtLogin(true), isTrue);
      expect(await port.registerCopypasteUri(), isTrue);
    },
  );

  test('rejects partial or invalid native state', () async {
    messenger.setMockMethodCallHandler(
      channel,
      (call) async => {'session': 'wayland', 'globalShortcuts': true},
    );

    await expectLater(port.status(), throwsFormatException);
  });
}
