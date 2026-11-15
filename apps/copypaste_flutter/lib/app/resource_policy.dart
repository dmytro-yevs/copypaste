import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../platform/lifecycle/native_memory_relief.dart';

/// Per-engine cache ownership limits. These do not bound live images, surfaces
/// or total process memory; native process-family qualification remains required.
abstract final class AppResourcePolicy {
  static const decodedImageBytes = 16 * 1024 * 1024;
  static const decodedImageCount = 128;
  static const skiaResourceBytes = 16 * 1024 * 1024;

  static final _pressure = _MemoryPressureObserver();
  static bool _presentationSuspended = false;

  /// Called before either the main or Quick Paste engine starts presentation.
  static Future<void> configure() async {
    final cache = PaintingBinding.instance.imageCache;
    cache.maximumSizeBytes = decodedImageBytes;
    cache.maximumSize = decodedImageCount;
    WidgetsBinding.instance
      ..removeObserver(_pressure)
      ..addObserver(_pressure);
    await _setRendererCacheBytes(skiaResourceBytes);
  }

  /// Releases decoded images, renderer resources and freed allocator pages
  /// after a suspended presentation has disposed its image listeners.
  static Future<void> suspendPresentation() async {
    _presentationSuspended = true;
    final cache = PaintingBinding.instance.imageCache;
    cache.clear();
    cache.clearLiveImages();
    await _releaseCaches();
  }

  /// Restores the renderer budget before a suspended presentation rebuilds.
  static Future<void> resumePresentation() async {
    _presentationSuspended = false;
    await _setRendererCacheBytes(skiaResourceBytes);
  }

  static Future<void> _releaseCaches() async {
    // A zero budget purges unlocked GPU resources. Both requests are queued
    // in order, so a visible presentation keeps its budget for the next frame.
    final requests = [
      _setRendererCacheBytes(0),
      if (!_presentationSuspended) _setRendererCacheBytes(skiaResourceBytes),
    ];
    await Future.wait(requests);
    await NativeMemoryRelief.releaseFreedMemory();
  }

  static Future<void> _setRendererCacheBytes(int bytes) async {
    try {
      // The engine acknowledges this channel even on Impeller, where it has no
      // GrContext to limit. Do not treat the reply as proof of enforcement.
      // macOS explicitly selects Skia in Info.plist; measure other backends.
      await SystemChannels.skia.invokeMethod<Object?>(
        'Skia.setResourceCacheMaxBytes',
        bytes,
      );
    } on MissingPluginException {
      // A renderer without this optional channel still gets the decoded cap.
    } on PlatformException {
      // Cache configuration must not make application functionality unavailable.
      // Missing enforcement remains visible in native resource qualification.
    }
  }
}

/// The framework already clears the decoded-image cache on memory pressure.
class _MemoryPressureObserver with WidgetsBindingObserver {
  @override
  void didHaveMemoryPressure() => unawaited(AppResourcePolicy._releaseCaches());
}
