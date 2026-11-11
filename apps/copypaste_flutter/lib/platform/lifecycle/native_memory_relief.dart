import 'package:flutter/services.dart';

/// Asks the host to return memory its allocator has already freed.
/// Hosts without a measured allocator policy do not register the channel.
abstract final class NativeMemoryRelief {
  static const _channel = MethodChannel('com.copypaste.app/memory');

  static Future<void> releaseFreedMemory() async {
    try {
      await _channel.invokeMethod<void>('releaseFreedMemory');
    } on MissingPluginException {
      // No allocator relief is registered on this host.
    } on PlatformException {
      // Relief is an optimization and never blocks presentation.
    }
  }
}
