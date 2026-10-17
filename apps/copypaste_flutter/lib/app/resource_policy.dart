import 'package:flutter/painting.dart';
import 'package:flutter/services.dart';

/// Per-engine cache ownership limits. These do not bound live images, surfaces
/// or total process memory; native process-family qualification remains required.
abstract final class AppResourcePolicy {
  static const decodedImageBytes = 16 * 1024 * 1024;
  static const decodedImageCount = 128;
  static const skiaResourceBytes = 16 * 1024 * 1024;

  /// Called before either the main or Quick Paste engine starts presentation.
  static Future<void> configure() async {
    final cache = PaintingBinding.instance.imageCache;
    cache.maximumSizeBytes = decodedImageBytes;
    cache.maximumSize = decodedImageCount;
    try {
      // The engine acknowledges this channel even on Impeller, where it has no
      // GrContext to limit. Do not treat the reply as proof of enforcement.
      // macOS explicitly selects Skia in Info.plist; measure other backends.
      await SystemChannels.skia.invokeMethod<Object?>(
        'Skia.setResourceCacheMaxBytes',
        skiaResourceBytes,
      );
    } on MissingPluginException {
      // A renderer without this optional channel still gets the decoded cap.
    } on PlatformException {
      // Cache configuration must not make application functionality unavailable.
      // Missing enforcement remains visible in native resource qualification.
    }
  }
}
