import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:path_provider/path_provider.dart';

import 'capture_notification_preview.dart';

abstract interface class CaptureNotificationPort {
  Future<void> initialize();

  Future<bool> requestPermission();

  Future<void> showCaptured({CaptureNotificationPreview? preview});
}

class PlatformCaptureNotificationPort implements CaptureNotificationPort {
  PlatformCaptureNotificationPort({FlutterLocalNotificationsPlugin? plugin})
    : _plugin = plugin ?? FlutterLocalNotificationsPlugin();

  static const _androidCapture = MethodChannel(
    'com.copypaste.app/android_capture',
  );
  final FlutterLocalNotificationsPlugin _plugin;
  bool _initialized = false;
  File? _previewFile;

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
  Future<void> showCaptured({CaptureNotificationPreview? preview}) async {
    if (Platform.isAndroid) return;
    await initialize();
    try {
      final previous = _previewFile;
      _previewFile = null;
      if (previous != null && await previous.exists()) await previous.delete();
      final image = preview?.imagePng;
      if (image != null) {
        final directory = await getTemporaryDirectory();
        final file = File('${directory.path}/copypaste-capture-preview.png');
        await file.writeAsBytes(image, flush: true);
        _previewFile = file;
      }
    } catch (_) {
      // An unavailable image cache must not suppress the notification text.
    }
    final imagePath = _previewFile?.path;
    await _plugin.show(
      id: 2208,
      title: 'Clipboard saved',
      body: preview?.text ?? 'A new item was added to CopyPaste.',
      notificationDetails: NotificationDetails(
        macOS: DarwinNotificationDetails(
          presentSound: false,
          attachments: imagePath == null
              ? null
              : [DarwinNotificationAttachment(imagePath)],
        ),
        windows: WindowsNotificationDetails(
          audio: WindowsNotificationAudio.silent(),
          images: imagePath == null
              ? const []
              : [WindowsImage(Uri.file(imagePath), altText: preview!.text)],
        ),
      ),
    );
  }
}

class NotificationPermissionDenied implements Exception {
  const NotificationPermissionDenied();
}
