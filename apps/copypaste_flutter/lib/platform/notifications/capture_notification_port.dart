import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';

abstract interface class CaptureNotificationPort {
  Future<void> initialize();

  Future<bool> requestPermission();

  Future<void> showCaptured();
}

class PlatformCaptureNotificationPort implements CaptureNotificationPort {
  PlatformCaptureNotificationPort({FlutterLocalNotificationsPlugin? plugin})
    : _plugin = plugin ?? FlutterLocalNotificationsPlugin();

  static const _androidCapture = MethodChannel(
    'com.copypaste.app/android_capture',
  );
  final FlutterLocalNotificationsPlugin _plugin;
  bool _initialized = false;

  @override
  Future<void> initialize() async {
    if (_initialized || Platform.isAndroid) return;
    await _plugin.initialize(
      settings: const InitializationSettings(
        macOS: DarwinInitializationSettings(
          requestAlertPermission: false,
          requestBadgePermission: false,
          requestSoundPermission: false,
        ),
        windows: WindowsInitializationSettings(
          appName: 'CopyPaste',
          appUserModelId: 'com.copypaste.app',
          guid: 'f112c130-c009-4a8c-b95a-16608410ce76',
        ),
      ),
    );
    _initialized = true;
  }

  @override
  Future<bool> requestPermission() async {
    if (Platform.isAndroid) {
      final state = await _androidCapture.invokeMapMethod<String, Object?>(
        'requestNotifications',
      );
      return state?['notificationGranted'] == true;
    }
    await initialize();
    if (Platform.isMacOS) {
      return await _plugin
              .resolvePlatformSpecificImplementation<
                MacOSFlutterLocalNotificationsPlugin
              >()
              ?.requestPermissions(alert: true, badge: false, sound: false) ??
          false;
    }
    return Platform.isWindows;
  }

  @override
  Future<void> showCaptured() async {
    if (Platform.isAndroid) return;
    await initialize();
    await _plugin.show(
      id: 2208,
      title: 'Clipboard saved',
      body: 'A new item was added to CopyPaste.',
      notificationDetails: const NotificationDetails(
        macOS: DarwinNotificationDetails(presentSound: false),
        windows: WindowsNotificationDetails(),
      ),
    );
  }
}

class NotificationPermissionDenied implements Exception {
  const NotificationPermissionDenied();
}
