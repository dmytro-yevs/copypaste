import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/error_text.dart';
import '../../modules/controller/module_view_controller.dart';
import '../../modules/models/module_models.dart';
import '../../modules/view/module_icons.dart';
import '../../modules/view/module_view_host.dart';
import '../controller/history_controller.dart';
import '../controller/history_module_actions.dart';
import '../models/history_models.dart';

/// Menu entries for the module actions on [clip]; [keyPrefix] names the
/// surface in their keys.
List<MenuItem> historyModuleActionMenuItems(
  BuildContext context, {
  required HistoryController controller,
  required HistoryClip clip,
  required Iterable<HistoryModuleClipAction> actions,
  required String keyPrefix,
}) => [
  for (final action in actions)
    MenuButton(
      key: ValueKey<String>('$keyPrefix-${action.id}-${clip.id}'),
      onPressed: (_) => unawaited(
        runHistoryModuleAction(
          context,
          controller: controller,
          clip: clip,
          action: action,
        ),
      ),
      leading: AppIconFrame(
        size: AppIconSize.sm,
        child: Icon(historyModuleActionIcon(action), size: AppIconSize.sm),
      ),
      child: Text(action.action.title),
    ),
];

IconData historyModuleActionIcon(HistoryModuleClipAction action) =>
    action.action.icon?.data ?? LucideIcons.puzzle;

/// Runs a module's item action with the clip's id and the text History
/// shows for it. A view opens in a drawer; other results are presented.
Future<void> runHistoryModuleAction(
  BuildContext context, {
  required HistoryController controller,
  required HistoryClip clip,
  required HistoryModuleClipAction action,
}) async {
  final actions = controller.moduleActions;
  if (actions == null) return;
  final modules = actions.modules;
  final moduleId = action.module.id;
  final title = action.action.title;
  ModuleResult result;
  try {
    result = await controller.runModuleAction(() async {
      final detail = await controller.capabilities.detail(clip.id);
      return modules.request(moduleId, action.action.command, {
        'item_id': clip.id,
        'text': detail.body ?? detail.preview,
      });
    });
  } catch (error) {
    if (!context.mounted) return;
    AppToast.show(
      context,
      title: context.l10n.historyActionFailedTitle,
      message: ErrorText.reason(error),
      tone: AppToastTone.error,
    );
    return;
  }
  if (!context.mounted) return;
  final view = result.view;
  if (view == null) {
    await presentModuleResult(
      context,
      result,
      title: title,
      moduleId: moduleId,
      services: actions.services,
    );
    return;
  }
  await showModuleViewDrawer(
    context,
    title: title,
    controller: ModuleViewController(
      modules: modules,
      moduleId: moduleId,
      view: view,
      transient: true,
    ),
    services: actions.services,
  );
}
