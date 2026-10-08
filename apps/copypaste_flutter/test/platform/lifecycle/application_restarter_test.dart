import 'package:copypaste_flutter/platform/lifecycle/application_restarter.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('com.copypaste.app/lifecycle');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  test(
    'requests a native process restart and propagates launch failure',
    () async {
      final calls = <String>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        calls.add(call.method);
        throw PlatformException(code: 'restart_failed');
      });
      await expectLater(
        const ApplicationRestarter().restart(),
        throwsA(isA<PlatformException>()),
      );
      expect(calls, ['restart']);
    },
  );
}
