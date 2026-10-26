import 'dart:async';
import 'dart:io';

import 'package:flutter_local_notifications/flutter_local_notifications.dart';

import 'app_notification_service.dart';

abstract interface class PairingNotificationPort {
  Stream<String> get opened;
  Future<void> showRequest({
    required String requestId,
    required String peerName,
  });
  Future<void> dismiss();
  Future<bool> requestPermission();
}

class PlatformPairingNotificationPort implements PairingNotificationPort {
  PlatformPairingNotificationPort({AppNotificationService? service})
    : _service = service ?? AppNotificationService.instance;
  final AppNotificationService _service;
  static const _id = 2209;
  static const _prefix = 'pairing:';
  @override
  Stream<String> get opened => _service.responses
      .where((payload) => payload.startsWith(_prefix))
      .map((payload) => payload.substring(_prefix.length));

  @override
  Future<void> showRequest({
    required String requestId,
    required String peerName,
  }) async {
    await _service.initialize();
    await _service.plugin.show(
      id: _id,
      title: 'Connection request',
      body:
          '$peerName wants to connect. Open CopyPaste to compare the code and confirm.',
      payload: '$_prefix$requestId',
      notificationDetails: const NotificationDetails(
        android: AndroidNotificationDetails(
          'pairing_requests',
          'Device connection requests',
          channelDescription: 'Requests to connect nearby devices',
          importance: Importance.high,
          priority: Priority.high,
          timeoutAfter: 60000,
        ),
        macOS: DarwinNotificationDetails(
          presentAlert: true,
          presentSound: true,
        ),
        windows: WindowsNotificationDetails(),
        linux: LinuxNotificationDetails(),
      ),
    );
  }

  @override
  Future<bool> requestPermission() async {
    await _service.initialize();
    if (Platform.isMacOS) {
      return await _service.plugin
              .resolvePlatformSpecificImplementation<
                MacOSFlutterLocalNotificationsPlugin
              >()
              ?.requestPermissions(alert: true, badge: false, sound: true) ??
          false;
    }
    if (Platform.isAndroid) {
      return await _service.plugin
              .resolvePlatformSpecificImplementation<
                AndroidFlutterLocalNotificationsPlugin
              >()
              ?.requestNotificationsPermission() ??
          false;
    }
    return Platform.isWindows || Platform.isLinux;
  }

  @override
  Future<void> dismiss() async {
    await _service.initialize();
    await _service.plugin.cancel(id: _id);
  }
}
