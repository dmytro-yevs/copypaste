import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/platform/security/device_authenticator.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' show Locale;
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/device_auth');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  final calls = <MethodCall>[];
  late Future<Object?> Function(MethodCall call) reply;

  setUp(() {
    calls.clear();
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    messenger.setMockMethodCallHandler(channel, (call) {
      calls.add(call);
      return reply(call);
    });
  });

  tearDown(() {
    messenger.setMockMethodCallHandler(channel, null);
    debugDefaultTargetPlatformOverride = null;
  });

  test('maps native outcomes and fails closed', () async {
    final authenticator = MethodChannelDeviceAuthenticator(channel: channel);
    final outcomes = {
      'success': DeviceAuthResult.success,
      'cancelled': DeviceAuthResult.cancelled,
      'unavailable': DeviceAuthResult.unavailable,
      'lockedOut': DeviceAuthResult.lockedOut,
      'unexpected': DeviceAuthResult.cancelled,
    };
    for (final MapEntry(:key, :value) in outcomes.entries) {
      reply = (_) async => key;
      expect(await authenticator.authenticate(reason: 'Unlock'), value);
    }
    expect(calls.last.arguments, {'reason': 'Unlock'});

    reply = (_) async => throw PlatformException(code: 'busy');
    expect(
      await authenticator.authenticate(reason: 'Unlock'),
      DeviceAuthResult.cancelled,
    );
    reply = (call) async =>
        call.method == 'availability' ? 'notConfigured' : null;
    expect(
      await authenticator.availability(),
      DeviceAuthAvailability.notConfigured,
    );
  });

  test('a missing native adapter reports unavailable', () async {
    messenger.setMockMethodCallHandler(channel, null);
    final authenticator = MethodChannelDeviceAuthenticator(channel: channel);
    expect(
      await authenticator.availability(),
      DeviceAuthAvailability.unsupported,
    );
    expect(
      await authenticator.authenticate(reason: 'Unlock'),
      DeviceAuthResult.unavailable,
    );
  });

  test('Linux is unsupported without calling the platform', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    final authenticator = MethodChannelDeviceAuthenticator(channel: channel);
    expect(
      await authenticator.availability(),
      DeviceAuthAvailability.unsupported,
    );
    expect(
      await authenticator.authenticate(reason: 'Unlock'),
      DeviceAuthResult.unavailable,
    );
    expect(calls, isEmpty);
  });

  test('concurrent requests share one prompt', () async {
    final pending = Completer<Object?>();
    reply = (_) => pending.future;
    final authenticator = MethodChannelDeviceAuthenticator(channel: channel);
    final first = authenticator.authenticate(reason: 'Unlock');
    final second = authenticator.authenticate(reason: 'Copy');
    pending.complete('success');
    expect(await Future.wait([first, second]), [
      DeviceAuthResult.success,
      DeviceAuthResult.success,
    ]);
    expect(calls, hasLength(1));
  });

  test('macOS receives the clause it embeds in its own sentence', () {
    final l10n = lookupAppLocalizations(const Locale('en'));
    expect(
      deviceAuthReason(
        l10n,
        DeviceAuthPurpose.unlockSecrets,
        platform: TargetPlatform.macOS,
      ),
      'unlock secret items',
    );
    expect(
      deviceAuthReason(
        l10n,
        DeviceAuthPurpose.unlockSecrets,
        platform: TargetPlatform.windows,
      ),
      'Unlock secret items',
    );
  });
}
