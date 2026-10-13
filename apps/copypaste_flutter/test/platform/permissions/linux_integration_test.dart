import 'package:copypaste_flutter/platform/permissions/linux_integration.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/linux_integration');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  final calls = <MethodCall>[];
  late Future<bool> Function() clipboardAvailable;
  late MethodChannelLinuxIntegrationPort port;

  setUp(() {
    calls.clear();
    clipboardAvailable = () async => true;
    port = MethodChannelLinuxIntegrationPort(
      channel: channel,
      clipboardAvailable: () => clipboardAvailable(),
    );
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call);
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
        'publishRuntimeEndpoint' => true,
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

  test('publishes only an absolute runtime endpoint', () async {
    await port.publishRuntimeEndpoint('/tmp/copypaste.sock');
    expect(calls.last.method, 'publishRuntimeEndpoint');
    expect(calls.last.arguments, {'endpoint': '/tmp/copypaste.sock'});

    await expectLater(
      port.publishRuntimeEndpoint('relative.sock'),
      throwsArgumentError,
    );
  });

  test('rejects partial or invalid native state', () async {
    messenger.setMockMethodCallHandler(
      channel,
      (call) async => {'session': 'wayland', 'globalShortcuts': true},
    );

    await expectLater(port.status(), throwsFormatException);
  });

  test('fails closed when the daemon clipboard is unavailable', () async {
    clipboardAvailable = () async => false;

    final status = await port.status();

    expect(status.session, LinuxDesktopSession.wayland);
    expect(status.globalShortcuts, isTrue);
    expect(status.remoteDesktop, LinuxRemoteDesktopState.active);
    expect(status.companion, LinuxCompanionState.active);
    expect(status.clipboard, isFalse);
    expect(status.quickPaste, isFalse);
  });

  test('fails closed when the daemon clipboard check throws', () async {
    clipboardAvailable = () async => throw StateError('daemon unavailable');

    final status = await port.status();

    expect(status.clipboard, isFalse);
    expect(status.quickPaste, isFalse);
  });

  test(
    'restores readiness when the daemon clipboard becomes available',
    () async {
      var available = false;
      clipboardAvailable = () async => available;

      expect((await port.status()).clipboard, isFalse);
      available = true;

      final status = await port.status();
      expect(status.clipboard, isTrue);
      expect(status.quickPaste, isTrue);
    },
  );
}
