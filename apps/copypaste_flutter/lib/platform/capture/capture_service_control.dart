import 'dart:io';

import 'package:flutter/services.dart';

abstract interface class CaptureServiceControl {
  Future<void> setPaused(bool paused);
}

class PlatformCaptureServiceControl implements CaptureServiceControl {
  const PlatformCaptureServiceControl();

  static const _androidChannel = MethodChannel(
    'com.copypaste.app/android_capture',
  );

  @override
  Future<void> setPaused(bool paused) async {
    if (!Platform.isAndroid) return;
    final state = await _androidChannel.invokeMapMethod<String, Object?>(
      paused ? 'stopCapture' : 'startCapture',
    );
    if (!paused && state?['startRequested'] != true) {
      throw const CaptureServiceUnavailable();
    }
  }
}

class CaptureServiceUnavailable implements Exception {
  const CaptureServiceUnavailable();
}
