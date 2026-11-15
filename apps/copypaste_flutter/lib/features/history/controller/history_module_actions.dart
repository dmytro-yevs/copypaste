import 'package:flutter/foundation.dart';

import '../../modules/controller/module_view_services.dart';
import '../../modules/controller/modules_controller.dart';
import '../../modules/models/module_models.dart';
import '../../modules/models/module_surfaces.dart';
import '../models/history_models.dart';

/// An action an enabled module offers for text clips.
class HistoryModuleClipAction {
  const HistoryModuleClipAction({required this.module, required this.action});

  final InstalledModule module;
  final ModuleItemAction action;

  /// Stable part of the action's widget keys.
  String get id => 'module-${module.id}-${action.id}';
}

/// The item actions of enabled modules, which History offers for text clips
/// beside its own actions.
class HistoryModuleActions extends ChangeNotifier {
  HistoryModuleActions({
    required this.modules,
    this.services = const ModuleViewServices(),
  }) {
    modules.addListener(notifyListeners);
  }

  final ModulesController modules;
  final ModuleViewServices services;

  List<HistoryModuleClipAction> actionsFor(HistoryClip clip) => [
    if (clip.contentKind.isTextual)
      for (final module in modules.activeModules)
        for (final action in module.surfaces.itemActions)
          HistoryModuleClipAction(module: module, action: action),
  ];

  @override
  void dispose() {
    modules.removeListener(notifyListeners);
    super.dispose();
  }
}
