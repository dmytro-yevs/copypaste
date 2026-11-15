import 'package:copypaste_flutter/platform/macos/macos_setup_gateway.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/macos_setup');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  test(
    'missing system record is distinct from unsupported autostart',
    () async {
      messenger.setMockMethodCallHandler(channel, (call) async => 'not_found');
      final gateway = MethodChannelMacosSetupGateway(channel: channel);

      expect(await gateway.loginItemStatus(), MacosLoginItemStatus.notFound);
    },
  );

  test(
    'enabling a missing record sends registration through the channel',
    () async {
      final calls = <MethodCall>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        calls.add(call);
        return call.method == 'loginItemStatus' ? 'not_found' : 'enabled';
      });
      final gateway = MethodChannelMacosSetupGateway(channel: channel);

      expect(await gateway.loginItemStatus(), MacosLoginItemStatus.notFound);
      expect(
        await gateway.setLaunchAtLogin(true),
        MacosLoginItemStatus.enabled,
      );
      expect(calls.last.method, 'setLaunchAtLogin');
      expect(calls.last.arguments, {'enabled': true});
    },
  );
}
