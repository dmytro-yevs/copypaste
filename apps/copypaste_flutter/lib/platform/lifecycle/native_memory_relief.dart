import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Asks the host to return memory its allocator has already freed.
/// macOS, Windows and Android implement it; Linux has no allocator policy.
abstract final class NativeMemoryRelief {
  static const _channel = MethodChannel('com.copypaste.app/memory');

  static Future<void> releaseFreedMemory() async {
    if (defaultTargetPlatform == TargetPlatform.linux) return;
    try {
      await _channel.invokeMethod<void>('releaseFreedMemory');
    } on MissingPluginException {
      // An engine whose host registers no relief, such as a secondary context.
    } on PlatformException {
      // Relief is an optimization and never blocks presentation.
    }
  }
}
