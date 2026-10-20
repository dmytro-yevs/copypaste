import 'dart:async';

import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/shared/runtime_event_service.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('shares a lazy watch and closes it after the final listener', () async {
    var allocations = 0;
    var cancellations = 0;
    final source = StreamController<runtime.RuntimeEvent>();
    final service = RuntimeEventService(
      allocate: () async {
        allocations++;
        return BigInt.one;
      },
      cancel: (_) async {
        cancellations++;
      },
      watch: (_) => source.stream,
    );
    final first = service.events.listen((_) {});
    final second = service.events.listen((_) {});
    await Future<void>.delayed(Duration.zero);
    expect(allocations, 1);
    source.add(
      runtime.RuntimeEvent(
        kind: 'ready',
        itemCount: BigInt.zero,
        captured: false,
        runtimeWatch: true,
      ),
    );
    expect(await service.ready, isTrue);
    await first.cancel();
    expect(cancellations, 0);
    await second.cancel();
    await Future<void>.delayed(Duration.zero);
    expect(cancellations, 1);
    await source.close();
  });

  test('cancels a watch allocated after the last listener leaves', () async {
    final allocation = Completer<BigInt>();
    var cancellations = 0;
    var watches = 0;
    final service = RuntimeEventService(
      allocate: () => allocation.future,
      cancel: (_) async {
        cancellations++;
      },
      watch: (_) {
        watches++;
        return const Stream.empty();
      },
    );
    final listener = service.events.listen((_) {});
    await listener.cancel();
    allocation.complete(BigInt.one);
    await Future<void>.delayed(Duration.zero);
    expect(cancellations, 1);
    expect(watches, 0);
  });

  test('reports a disconnected watch before retry', () async {
    final source = StreamController<runtime.RuntimeEvent>();
    final service = RuntimeEventService(
      allocate: () async => BigInt.one,
      cancel: (_) async {},
      watch: (_) => source.stream,
    );
    final errors = <Object>[];
    final subscription = service.events.listen(
      (_) {},
      onError: (Object error) => errors.add(error),
    );
    await Future<void>.delayed(Duration.zero);
    source.add(
      runtime.RuntimeEvent(
        kind: 'ready',
        itemCount: BigInt.zero,
        captured: false,
        runtimeWatch: true,
      ),
    );
    await service.ready;
    await source.close();
    await Future<void>.delayed(Duration.zero);
    expect(errors, hasLength(1));
    await subscription.cancel();
  });
}
