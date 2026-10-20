import 'dart:io';

import 'package:flutter/services.dart';

class ScreenshotCaptureStatus {
  const ScreenshotCaptureStatus({
    this.enabled = true,
    this.mediaGranted = false,
    this.notificationGranted = false,
    this.running = false,
  });

  final bool enabled;
  final bool mediaGranted;
  final bool notificationGranted;
  final bool running;

  bool get needsPermission => !mediaGranted || !notificationGranted;
}

abstract interface class ScreenshotCapture {
  bool get supported;
  Future<ScreenshotCaptureStatus> status();
  Future<ScreenshotCaptureStatus> setEnabled(bool enabled);
  Future<ScreenshotCaptureStatus> requestPermission();
}

abstract interface class ScreenshotCaptureEvents {
  Stream<ScreenshotCaptureStatus> get changes;
}

class MethodChannelScreenshotCapture
    implements ScreenshotCapture, ScreenshotCaptureEvents {
  const MethodChannelScreenshotCapture({
    MethodChannel channel = const MethodChannel(
      'com.copypaste.app/android_capture',
    ),
    EventChannel events = const EventChannel(
      'com.copypaste.app/android_capture/screenshot_state',
    ),
  }) : _channel = channel,
       _events = events;

  final MethodChannel _channel;
  final EventChannel _events;

  @override
  Stream<ScreenshotCaptureStatus> get changes => _events
      .receiveBroadcastStream()
      .map((raw) => _decode(Map<String, Object?>.from(raw as Map)));

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
    return _decode(raw);
  }

  ScreenshotCaptureStatus _decode(Map<String, Object?> raw) =>
      ScreenshotCaptureStatus(
        enabled: raw['enabled'] as bool? ?? true,
        mediaGranted: raw['mediaGranted'] as bool? ?? false,
        notificationGranted: raw['notificationGranted'] as bool? ?? false,
        running: raw['running'] as bool? ?? false,
      );
}
