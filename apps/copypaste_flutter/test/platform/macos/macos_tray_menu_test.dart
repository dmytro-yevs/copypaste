import 'package:copypaste_flutter/platform/macos/macos_tray_menu.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/tray_menu');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  test('requests visibility for the existing native menu', () async {
    final calls = <MethodCall>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call);
      return null;
    });
    await showMacosTrayMenuImages(1234, channel: channel);
    expect(calls.single.method, 'showImages');
    expect(calls.single.arguments, {'nativeMenuAddress': 1234});
  });

  test(
    'native tray activation binds its retained icon and releases the binding',
    () async {
      final calls = <MethodCall>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        calls.add(call);
        return null;
      });
      await bindMacosTrayQuickPaste(4321, channel: channel);
      await unbindMacosTrayQuickPaste(channel: channel);
      expect(calls.map((call) => call.method), [
        'bindQuickPaste',
        'unbindQuickPaste',
      ]);
      expect(calls.first.arguments, {'nativeTrayAddress': 4321});
    },
  );
}
