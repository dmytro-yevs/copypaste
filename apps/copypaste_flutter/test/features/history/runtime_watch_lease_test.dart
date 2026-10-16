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

  test('a completed History Watch releases its ID for a replacement', () async {
    final port = _FakeRuntimeWatchPort();
    final lease = RuntimeWatchLease(
      allocate: port.allocate,
      cancel: port.cancel,
    );

    final first = await lease.watchId;
    await lease.release(first);
    final second = await lease.watchId;

    expect(second, isNot(first));
    expect(port.cancelled, [first]);
    await lease.dispose();
    expect(port.cancelled, [first, second]);
  });

  test(
    'a failed Watch allocation does not poison the replacement lease',
    () async {
      final port = _FakeRuntimeWatchPort()..failedAllocations = 1;
      final lease = RuntimeWatchLease(
        allocate: port.allocate,
        cancel: port.cancel,
      );

      await expectLater(lease.watchId, throwsStateError);
      await Future<void>.delayed(Duration.zero);

      expect(await lease.watchId, BigInt.one);
      await lease.dispose();
      expect(port.cancelled, [BigInt.one]);
    },
  );
}

class _FakeRuntimeWatchPort {
  BigInt _next = BigInt.one;
  int failedAllocations = 0;
  final List<BigInt> cancelled = [];

  Future<BigInt> allocate() async {
    if (failedAllocations > 0) {
      failedAllocations--;
      throw StateError('runtime unavailable');
    }
    final watchId = _next;
    _next += BigInt.one;
    return watchId;
  }

  Future<void> cancel(BigInt watchId) async {
    cancelled.add(watchId);
  }
}
