import 'package:flutter/services.dart';

/// The native policy owns persistence and every application window/activity.
abstract interface class ScreenshotProtection {
  Future<bool> blocked();
  Future<void> setBlocked(bool blocked);
}

class MethodChannelScreenshotProtection implements ScreenshotProtection {
  const MethodChannelScreenshotProtection({
    MethodChannel channel = const MethodChannel('com.copypaste.app/security'),
  }) : _channel = channel;

  final MethodChannel _channel;

  @override
  Future<bool> blocked() async {
    try {
      return await _channel.invokeMethod<bool>('getBlockScreenshots') ?? false;
    } on MissingPluginException {
      return false;
    }
  }

  @override
  Future<void> setBlocked(bool blocked) async {
    final applied = await _channel.invokeMethod<bool>('setBlockScreenshots', {
      'enabled': blocked,
    });
    if (applied != true) {
      throw PlatformException(code: 'screenshot_protection_failed');
    }
  }
}
