import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:path_provider/path_provider.dart';

import 'capture_notification_preview.dart';
import 'app_notification_service.dart';

abstract interface class CaptureNotificationPort {
  Future<void> initialize();

  Future<bool> requestPermission();

  Future<void> showCaptured({CaptureNotificationPreview? preview});
}

class PlatformCaptureNotificationPort implements CaptureNotificationPort {
  PlatformCaptureNotificationPort({FlutterLocalNotificationsPlugin? plugin})
    : _notifications = plugin == null
          ? AppNotificationService.instance
          : AppNotificationService(plugin: plugin);

  static const _androidCapture = MethodChannel(
    'com.copypaste.app/android_capture',
  );
  final AppNotificationService _notifications;
  FlutterLocalNotificationsPlugin get _plugin => _notifications.plugin;
  bool _initialized = false;
  File? _previewFile;

  @override
  Future<void> initialize() async {
    if (_initialized || Platform.isAndroid) return;
    await _notifications.initialize();
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
    return Platform.isWindows || Platform.isLinux;
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
        linux: LinuxNotificationDetails(
          icon: imagePath == null ? null : FilePathLinuxIcon(imagePath),
          suppressSound: true,
        ),
      ),
    );
  }
}

class NotificationPermissionDenied implements Exception {
  const NotificationPermissionDenied();
}
