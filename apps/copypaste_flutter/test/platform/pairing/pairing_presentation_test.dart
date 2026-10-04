import 'dart:async';

import 'package:copypaste_flutter/platform/pairing/pairing_presentation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const channel = MethodChannel('test/pairing-presentation');

  TestWidgetsFlutterBinding.ensureInitialized();

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  test(
    'does not open a context when protected presentation is unavailable',
    () async {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async {
            if (call.method == 'isSupported') {
              return false;
            }
            fail('must not open an unsupported protected context');
          });
      final controller = PairingPresentationController(
        host: MethodChannelPairingPresentationHost(channel: channel),
      );

      final lease = await controller.open(
        const PairingPresentationRequest(ceremonyId: 'ceremony-1'),
      );

      expect(lease, isNull);
    },
  );

  test('toggles capture protection through the native host channel', () async {
    final calls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call);
          return true;
        });
    final protection = MethodChannelPairingCaptureProtection(channel: channel);

    expect(await protection.setEnabled(true), isTrue);
    expect(await protection.setEnabled(false), isTrue);

    expect(calls.map((call) => call.method), [
      'setCaptureProtection',
      'setCaptureProtection',
    ]);
    expect(calls.first.arguments, <String, Object>{'enabled': true});
    expect(calls.last.arguments, <String, Object>{'enabled': false});
  });

  test(
    'opens and closes only an opaque protected context identifier',
    () async {
      final calls = <MethodCall>[];
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async {
            calls.add(call);
            if (call.method == 'isSupported') {
              return true;
            }
            if (call.method == 'open') {
              return <String, Object>{'contextId': 'context-1'};
            }
            return null;
          });
      final controller = PairingPresentationController(
        host: MethodChannelPairingPresentationHost(channel: channel),
      );

      final lease = await controller.open(
        const PairingPresentationRequest(ceremonyId: 'ceremony-1'),
      );
      await lease!.close();

      expect(lease.contextId, 'context-1');
      expect(calls.map((call) => call.method), <String>[
        'isSupported',
        'open',
        'close',
      ]);
      expect(calls[1].arguments, <String, Object>{'ceremonyId': 'ceremony-1'});
      expect(calls[2].arguments, <String, Object>{'contextId': 'context-1'});
    },
  );

  test(
    'does not retain material returned beside a protected context id',
    () async {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async {
            if (call.method == 'isSupported') return true;
            if (call.method == 'open') {
              return <String, Object>{
                'contextId': 'context-1',
                'artifact': 'must-not-cross-the-ordinary-channel',
              };
            }
            return true;
          });
      final host = MethodChannelPairingPresentationHost(channel: channel);

      final lease = await host.open(
        const PairingPresentationRequest(ceremonyId: 'ceremony-1'),
      );

      expect(lease?.contextId, 'context-1');
      await lease?.close();
    },
  );

  test('closes a late context when its ceremony is invalidated', () async {
    final host = _FakeHost();
    final controller = PairingPresentationController(host: host);

    final opening = controller.open(
      const PairingPresentationRequest(ceremonyId: 'ceremony-1'),
    );
    controller.invalidate();
    host.complete();

    expect(await opening, isNull);
    expect(host.lease.closed, isTrue);
  });
}

class _FakeHost implements PairingPresentationHost {
  final _FakeLease lease = _FakeLease();
  final Completer<PairingPresentationLease?> _completer =
      Completer<PairingPresentationLease?>();

  @override
  Future<PairingPresentationAvailability> availability() async =>
      PairingPresentationAvailability.available;

  @override
  Future<PairingPresentationLease?> open(PairingPresentationRequest request) =>
      _completer.future;

  void complete() => _completer.complete(lease);
}

class _FakeLease implements PairingPresentationLease {
  bool closed = false;

  @override
  String get contextId => 'context-1';

  @override
  Future<void> close() async {
    closed = true;
  }
}
