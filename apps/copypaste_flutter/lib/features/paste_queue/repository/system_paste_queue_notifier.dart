import 'dart:io';

import 'package:flutter_local_notifications/flutter_local_notifications.dart';

import '../../../l10n/l10n.dart';
import '../../../platform/notifications/app_notification_service.dart';
import '../controller/paste_queue_controller.dart';
import '../models/paste_queue_models.dart';
import '../presentation/paste_queue_messages.dart';

/// Reports shortcut-run queue feedback as one replaceable system
/// notification, so a press while another app has focus is never silent.
class SystemPasteQueueNotifier implements PasteQueueNotifier {
  SystemPasteQueueNotifier({AppNotificationService? service})
    : _service = service ?? AppNotificationService.instance;

  static const _id = 2214;

  final AppNotificationService _service;

  @override
  Future<void> show(PasteQueueFeedback feedback) async {
    if (Platform.isAndroid || Platform.isIOS) return;
    await _service.initialize();
    final l10n = AppStrings.current;
    await _service.plugin.show(
      id: _id,
      title: l10n.pasteQueueSettingsTitle,
      body: l10n.pasteQueueFeedback(feedback),
      notificationDetails: NotificationDetails(
        macOS: const DarwinNotificationDetails(presentSound: false),
        windows: WindowsNotificationDetails(
          audio: WindowsNotificationAudio.silent(),
        ),
        linux: const LinuxNotificationDetails(suppressSound: true),
      ),
    );
  }
}
