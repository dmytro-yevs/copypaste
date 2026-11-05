import 'package:copypaste_flutter/app/resource_policy.dart';
import 'package:flutter/painting.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  const relief = MethodChannel('com.copypaste.app/memory');

  test(
    'an unavailable renderer channel does not block presentation startup',
    () async {
      final cache = PaintingBinding.instance.imageCache;
      final oldBytes = cache.maximumSizeBytes;
      final oldCount = cache.maximumSize;
      addTearDown(() {
        messenger.setMockMethodCallHandler(SystemChannels.skia, null);
        cache.maximumSizeBytes = oldBytes;
        cache.maximumSize = oldCount;
      });
      for (final error in [
        MissingPluginException(),
        PlatformException(code: 'unsupported_renderer'),
      ]) {
        cache.maximumSizeBytes = 100 * 1024 * 1024;
        messenger.setMockMethodCallHandler(
          SystemChannels.skia,
          (_) async => throw error,
        );
        await expectLater(AppResourcePolicy.configure(), completes);
        expect(cache.maximumSizeBytes, lessThan(100 * 1024 * 1024));
        expect(cache.maximumSize, lessThan(1000));
      }
    },
  );

  test(
    'suspension and OS memory pressure release renderer and allocator caches',
    () async {
      final events = <Object?>[];
      messenger.setMockMethodCallHandler(SystemChannels.skia, (call) async {
        events.add(call.arguments);
        return true;
      });
      messenger.setMockMethodCallHandler(relief, (call) async {
        events.add(call.method);
        return null;
      });
      addTearDown(() async {
        await AppResourcePolicy.resumePresentation();
        messenger.setMockMethodCallHandler(SystemChannels.skia, null);
        messenger.setMockMethodCallHandler(relief, null);
      });
      await AppResourcePolicy.configure();
      events.clear();

      // Native hosts deliver OS pressure on the framework system channel.
      Future<void> memoryPressure() => messenger.handlePlatformMessage(
        SystemChannels.system.name,
        SystemChannels.system.codec.encodeMessage({'type': 'memoryPressure'}),
        (_) {},
      );

      await memoryPressure();
      await pumpEventQueue();
      expect(events, [
        0,
        AppResourcePolicy.skiaResourceBytes,
        'releaseFreedMemory',
      ]);

      events.clear();
      await AppResourcePolicy.suspendPresentation();
      await memoryPressure();
      await pumpEventQueue();
      expect(events, [0, 'releaseFreedMemory', 0, 'releaseFreedMemory']);

      events.clear();
      await AppResourcePolicy.resumePresentation();
      expect(events, [AppResourcePolicy.skiaResourceBytes]);
    },
  );
}
