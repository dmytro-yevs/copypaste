import 'dart:io';

import 'package:flutter/services.dart';

class ScreenshotCaptureStatus {
  const ScreenshotCaptureStatus({
    this.enabled = true,
    this.mediaGranted = false,
    this.notificationGranted = false,
    this.running = false,
    this.sourceAccessGranted = false,
  });

  final bool enabled;
  final bool mediaGranted;
  final bool notificationGranted;
  final bool running;
  final bool sourceAccessGranted;

  bool get needsPermission => !mediaGranted || !notificationGranted;
}

abstract interface class ScreenshotCapture {
  bool get supported;
  Future<ScreenshotCaptureStatus> status();
  Future<ScreenshotCaptureStatus> setEnabled(bool enabled);
  Future<ScreenshotCaptureStatus> requestPermission();
  Future<bool> openSourceAccess();
}

class MethodChannelScreenshotCapture implements ScreenshotCapture {
  const MethodChannelScreenshotCapture({
    MethodChannel channel = const MethodChannel(
      'com.copypaste.app/android_capture',
    ),
  }) : _channel = channel;

  final MethodChannel _channel;

  @override
  bool get supported => Platform.isAndroid;

  @override
  Future<ScreenshotCaptureStatus> status() => _invoke('screenshotState');

  @override
  Future<ScreenshotCaptureStatus> setEnabled(bool enabled) =>
      _invoke('setScreenshotCaptureEnabled', {'enabled': enabled});

  @override
  Future<ScreenshotCaptureStatus> requestPermission() =>
      _invoke('requestScreenshotPermission');

  @override
  Future<bool> openSourceAccess() async =>
      await _channel.invokeMethod<bool>('openScreenshotSourceAccess') ?? false;

  Future<ScreenshotCaptureStatus> _invoke(
    String method, [
    Map<String, Object?>? arguments,
  ]) async {
    final raw = await _channel.invokeMapMethod<String, Object?>(
      method,
      arguments,
    );
    if (raw == null) {
      throw PlatformException(code: 'screenshot_capture_unavailable');
    }
    return ScreenshotCaptureStatus(
      enabled: raw['enabled'] as bool? ?? true,
      mediaGranted: raw['mediaGranted'] as bool? ?? false,
      notificationGranted: raw['notificationGranted'] as bool? ?? false,
      running: raw['running'] as bool? ?? false,
      sourceAccessGranted: raw['sourceAccessGranted'] as bool? ?? false,
    );
  }
}
