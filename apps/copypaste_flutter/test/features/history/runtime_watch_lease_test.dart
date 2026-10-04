import 'package:copypaste_flutter/features/history/repository/runtime_history_repository.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test(
    'disposing History Watch leaves a second feature Watch active',
    () async {
      final port = _FakeRuntimeWatchPort();
      final history = RuntimeWatchLease(
        allocate: port.allocate,
        cancel: port.cancel,
      );
      final devices = RuntimeWatchLease(
        allocate: port.allocate,
        cancel: port.cancel,
      );

      final historyId = await history.watchId;
      final devicesId = await devices.watchId;
      await history.dispose();

      expect(historyId, isNot(devicesId));
      expect(port.cancelled, [historyId]);
      expect(await devices.watchId, devicesId);
      await devices.dispose();
      expect(port.cancelled, [historyId, devicesId]);
    },
  );
}

class _FakeRuntimeWatchPort {
  BigInt _next = BigInt.one;
  final List<BigInt> cancelled = [];

  Future<BigInt> allocate() async {
    final watchId = _next;
    _next += BigInt.one;
    return watchId;
  }

  Future<void> cancel(BigInt watchId) async {
    cancelled.add(watchId);
  }
}
