import 'package:copypaste_flutter/app/resource_policy.dart';
import 'package:flutter/painting.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'an unavailable renderer channel does not block presentation startup',
    () async {
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
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
}
