import 'dart:async';

import 'package:flutter_local_notifications/flutter_local_notifications.dart';

import '../../../l10n/l10n.dart';
import '../../../platform/notifications/app_notification_service.dart';
import '../models/ai_tools_models.dart';

/// A system notification for an AI tool request that arrives while the
/// main window is hidden; activating it reports the request id.
abstract interface class AiApprovalNotifier {
  Stream<String> get opened;

  Future<void> show(AiApprovalRequest request);

  Future<void> dismiss();
}

/// [AiApprovalNotifier] through the app's one notification plugin owner.
class SystemAiApprovalNotifier implements AiApprovalNotifier {
  SystemAiApprovalNotifier({AppNotificationService? service})
    : _service = service ?? AppNotificationService.instance;

  static const _id = 2216;
  static const _prefix = 'ai-approval:';

  final AppNotificationService _service;

  @override
  Stream<String> get opened => _service.responses
      .where((payload) => payload.startsWith(_prefix))
      .map((payload) => payload.substring(_prefix.length));

  @override
  Future<void> show(AiApprovalRequest request) async {
    await _service.initialize();
    final l10n = AppStrings.current;
    await _service.plugin.show(
      id: _id,
      title: switch (request.kind) {
        AiApprovalKind.access => l10n.aiToolsApprovalTitle(request.displayName),
        AiApprovalKind.volume => l10n.aiToolsVolumeTitle,
      },
      body: switch (request.kind) {
        AiApprovalKind.access => l10n.aiToolsNotificationBody,
        AiApprovalKind.volume => l10n.aiToolsVolumeMessage(request.displayName),
      },
      payload: '$_prefix${request.requestId}',
      notificationDetails: const NotificationDetails(
        macOS: DarwinNotificationDetails(presentAlert: true),
        windows: WindowsNotificationDetails(),
        linux: LinuxNotificationDetails(),
      ),
    );
  }

  @override
  Future<void> dismiss() async {
    await _service.initialize();
    await _service.plugin.cancel(id: _id);
  }
}
