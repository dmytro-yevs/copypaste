import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
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
    final strings = AppStrings.current;
    await _service.plugin.show(
      id: _id,
      title: strings.platformPairingNotificationTitle,
      body: strings.platformPairingNotificationBody(peerName),
      payload: '$_prefix$requestId',
      notificationDetails: NotificationDetails(
        android: AndroidNotificationDetails(
          'pairing_requests',
          strings.platformPairingNotificationChannelName,
          channelDescription:
              strings.platformPairingNotificationChannelDescription,
          importance: Importance.high,
          priority: Priority.high,
          timeoutAfter: 60000,
        ),
        macOS: const DarwinNotificationDetails(
          presentAlert: true,
          presentSound: true,
        ),
        windows: const WindowsNotificationDetails(),
        linux: const LinuxNotificationDetails(),
      ),
    );
  }

  @override
  Future<bool> requestPermission() => _service.requestPermission(sound: true);

  @override
  Future<void> dismiss() async {
    await _service.initialize();
    await _service.plugin.cancel(id: _id);
  }
}
