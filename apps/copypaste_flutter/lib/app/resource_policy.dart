import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../platform/lifecycle/native_memory_relief.dart';

/// Per-engine cache ownership limits. These do not bound live images, surfaces
/// or total process memory; native process-family qualification remains required.
abstract final class AppResourcePolicy {
  static const decodedImageBytes = 16 * 1024 * 1024;
  static const decodedImageCount = 128;
  static const skiaResourceBytes = 16 * 1024 * 1024;

  /// Frames rendered after a suspension on macOS, and the gap between them.
  static const settlingFrames = 4;
  static const settlingInterval = Duration(milliseconds: 300);

  /// Collections one suspension runs. The first frees the unmounted
  /// presentation; the later ones let the VM move what survived off sparse
  /// pages, which is what leaves whole pages to return.
  static const collectionCycles = 4;

  /// Most steps one collection spends on reaching its threshold, and their gap.
  static const collectionSteps = 48;
  static const collectionInterval = Duration(milliseconds: 20);
  static const _collectionStepBytes = 4 * 1024 * 1024;
  static const _collectionResidentAllowance = 16 * 1024 * 1024;

  static final _pressure = _MemoryPressureObserver();
  static bool _presentationSuspended = false;
  static int _suspension = 0;

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
    final suspension = ++_suspension;
    final cache = PaintingBinding.instance.imageCache;
    cache.clear();
    cache.clearLiveImages();
    await _releaseCaches();
    // Measured on macOS only; other hosts keep the cache release above.
    if (defaultTargetPlatform != TargetPlatform.macOS) return;
    for (var cycle = 0; cycle < collectionCycles; cycle++) {
      if (!await _collectOldSpace(suspension)) break;
      await Future<void>.delayed(settlingInterval);
    }
    await _settleWindowSurface(suspension);
    // Native peers the collection finalized are freed allocator pages now.
    if (_stillSuspended(suspension)) {
      await NativeMemoryRelief.releaseFreedMemory();
    }
  }

  static bool _stillSuspended(int suspension) =>
      _presentationSuspended && suspension == _suspension;

  /// Has the Dart VM collect old space now instead of when the heap next
  /// grows, and reports whether a collection completed.
  ///
  /// The VM starts that collection only once old space has grown past its
  /// threshold, and a hidden application allocates nothing. Typed data this
  /// large gets pages of its own that the VM never writes, so each step counts
  /// as growth without being paged in. The first step is the probe: it dies
  /// in the collection that frees everything else unreachable. A resident
  /// size that rises anyway means the pages were written, and ends the steps.
  static Future<bool> _collectOldSpace(int suspension) async {
    final ceiling = ProcessInfo.currentRss + _collectionResidentAllowance;
    final steps = <WeakReference<Object>>[];
    do {
      steps.add(WeakReference(Uint8List(_collectionStepBytes)));
      await Future<void>.delayed(collectionInterval);
    } while (steps.first.target != null &&
        steps.length < collectionSteps &&
        ProcessInfo.currentRss < ceiling &&
        _stillSuspended(suspension));
    return steps.first.target == null;
  }

  /// Renders the frames that let macOS drop the window surface replaced by the
  /// suspended presentation, and that give the VM the idle notifications it
  /// returns freed heap pages on. Hidden lifecycle states disable ordinary
  /// frames, so the single frame that swaps the presentation does neither.
  static Future<void> _settleWindowSurface(int suspension) async {
    for (var frame = 0; frame < settlingFrames; frame++) {
      if (!_stillSuspended(suspension)) return;
      WidgetsBinding.instance.scheduleForcedFrame();
      await Future<void>.delayed(settlingInterval);
    }
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
