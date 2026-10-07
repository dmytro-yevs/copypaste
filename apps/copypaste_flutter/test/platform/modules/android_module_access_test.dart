import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/platform/modules/android_module_access.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const sms = MethodChannel('test/sms_modules');
  const capture = MethodChannel('com.copypaste.app/android_capture');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  const access = AndroidModuleAccess(channel: sms);
  final calls = <String>[];
  var notifications = false;

  Map<String, Object> state() => {
    'smsGranted': true,
    'notificationGranted': notifications,
    'adbCommands': 'adb shell sms',
    'shizuku': {
      'supported': true,
      'installed': true,
      'running': false,
      'permission': false,
    },
  };

  setUp(() {
    calls.clear();
    notifications = false;
    messenger.setMockMethodCallHandler(sms, (call) async {
      calls.add(call.method);
      return call.method == 'synchronize' ? true : state();
    });
    messenger.setMockMethodCallHandler(capture, (call) async {
      calls.add(call.method);
      if (call.method == 'openShizuku') return true;
      notifications = true;
      return <String, Object>{};
    });
  });

  tearDown(() {
    messenger.setMockMethodCallHandler(sms, null);
    messenger.setMockMethodCallHandler(capture, null);
  });

  test('SMS access requires both SMS grants and notifications', () async {
    final current = await access.smsState();
    expect(current.smsGranted, isTrue);
    expect(current.notificationGranted, isFalse);
    expect(current.granted, isFalse);
    expect(current.shizuku.installed, isTrue);
    expect(current.shizuku.running, isFalse);
    final granted = await access.requestSmsNotifications();
    expect(granted.granted, isTrue);
    expect(calls, ['state', 'requestNotifications', 'state']);
  });

  test('opens Shizuku through the existing Android setup gateway', () async {
    expect(await access.openShizuku(), isTrue);
    expect(calls, ['openShizuku']);
  });

  test('applies SMS grants through the SMS channel only', () async {
    await access.configureSms();
    expect(await access.synchronize(), isTrue);
    expect(calls, ['configure', 'synchronize']);
  });

  test('incomplete native state never claims verified access', () async {
    messenger.setMockMethodCallHandler(
      sms,
      (_) async => {'granted': true, 'adbCommands': 'adb shell sms'},
    );
    await expectLater(access.smsState(), throwsA(isA<ModulesException>()));
  });

  test('native grant refusal preserves its actionable explanation', () async {
    messenger.setMockMethodCallHandler(sms, (_) async {
      throw PlatformException(
        code: 'sms_access_denied',
        message: 'SMS access was not granted. Use ADB setup.',
      );
    });
    await expectLater(
      access.configureSms(),
      throwsA(
        isA<ModulesException>().having(
          (error) => error.message,
          'message',
          contains('Use ADB setup'),
        ),
      ),
    );
  });
}
