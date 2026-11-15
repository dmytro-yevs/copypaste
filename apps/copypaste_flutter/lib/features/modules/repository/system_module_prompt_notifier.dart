import 'package:flutter_local_notifications/flutter_local_notifications.dart';

import '../../../l10n/l10n.dart';
import '../../../platform/notifications/app_notification_service.dart';
import '../models/module_prompt.dart';
import 'modules_repository.dart';

/// [ModulePromptNotifier] through the app's one notification plugin owner.
class SystemModulePromptNotifier implements ModulePromptNotifier {
  SystemModulePromptNotifier({AppNotificationService? service})
    : _service = service ?? AppNotificationService.instance;

  static const _id = 2216;
  static const _prefix = 'module-prompt:';

  final AppNotificationService _service;

  @override
  Stream<String> get opened => _service.responses
      .where((payload) => payload.startsWith(_prefix))
      .map((payload) => payload.substring(_prefix.length));

  @override
  Future<void> show(ModulePrompt prompt) async {
    await _service.initialize();
    final strings = AppStrings.current;
    await _service.plugin.show(
      id: _id,
      title: prompt.title,
      body: strings.modulesPromptNotificationBody,
      payload: '$_prefix${prompt.id}',
      notificationDetails: NotificationDetails(
        android: AndroidNotificationDetails(
          'module_prompts',
          strings.settingsSectionModules,
          importance: Importance.high,
          priority: Priority.high,
        ),
        macOS: const DarwinNotificationDetails(presentAlert: true),
        windows: const WindowsNotificationDetails(),
        linux: const LinuxNotificationDetails(),
      ),
    );
  }

  @override
  Future<void> dismiss() async {
    await _service.initialize();
    await _service.plugin.cancel(id: _id);
  }
}
