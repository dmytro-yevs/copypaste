import 'dart:async';

import 'package:copypaste_flutter/platform/android/android_capture_setup_gateway.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'capture state events decode the initial snapshot and unsubscribe',
    () async {
      const channel = EventChannel('test/android-capture-state');
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      var cancelled = false;
      messenger.setMockStreamHandler(
        channel,
        MockStreamHandler.inline(
          onListen: (_, events) => events.success({
            'packageName': 'com.copypaste.app',
            'privilegedGrants': true,
            'notificationGranted': true,
            'batteryExempt': false,
            'captureEnabled': true,
            'serviceRunning': true,
            'lastCaptureAtMs': 7,
            'observedAtMs': 8,
            'shizuku': {
              'supported': true,
              'installed': true,
              'running': true,
              'permission': true,
            },
            'adbCommands': <String>['cmd appops set'],
          }),
          onCancel: (_) => cancelled = true,
        ),
      );
      addTearDown(() => messenger.setMockStreamHandler(channel, null));
      final gateway = MethodChannelAndroidCaptureSetupGateway(events: channel);
      final received = Completer<AndroidCaptureSetupState>();
      final subscription = gateway.changes.listen(received.complete);
      final state = await received.future;
      expect(state.backgroundCaptureRunning, isTrue);
      expect(state.lastCaptureAtMs, 7);
      expect(state.shizuku.permission, isTrue);
      await subscription.cancel();
      expect(cancelled, isTrue);
    },
  );

  test('refused clipboard intake reports false instead of failing', () async {
    const channel = MethodChannel('test/android-capture');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(
      channel,
      (_) async => throw PlatformException(code: 'capture_admission_refused'),
    );
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    final gateway = MethodChannelAndroidCaptureSetupGateway(channel: channel);
    expect(await gateway.setForegroundCaptureEnabled(true), isFalse);
  });
}
