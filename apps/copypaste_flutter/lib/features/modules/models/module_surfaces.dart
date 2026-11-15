import 'module_view.dart';

/// Places in the app where the host shows a module.
class ModuleSurfaces {
  const ModuleSurfaces({
    this.settings,
    this.quickPaste,
    this.itemActions = const [],
    this.trayActions = const [],
  });

  /// A top-level Settings section titled like the module.
  final ModuleSettingsSurface? settings;
  final ModuleQuickPasteSurface? quickPaste;

  /// Actions on text history items.
  final List<ModuleItemAction> itemActions;

  /// Desktop tray entries; each opens the settings section with a drawer.
  final List<ModuleTrayAction> trayActions;
}

class ModuleSettingsSurface {
  const ModuleSettingsSurface({required this.command, this.icon});

  /// Takes no arguments and returns a view.
  final String command;
  final ModuleIcon? icon;
}

class ModuleQuickPasteSurface {
  const ModuleQuickPasteSurface({
    required this.title,
    required this.searchCommand,
    required this.activateCommand,
    this.mixedLimit = 0,
  });
  final String title;

  /// Takes `query` and returns results; an empty query lists the tab.
  final String searchCommand;

  /// Takes `id` and returns a delivery, or a view whose form submit does.
  final String activateCommand;

  /// Results shown above history for a non-empty query.
  final int mixedLimit;
}

class ModuleItemAction {
  const ModuleItemAction({
    required this.id,
    required this.title,
    required this.command,
    this.icon,
  });
  final String id;
  final String title;

  /// Takes `item_id` and `text`.
  final String command;
  final ModuleIcon? icon;
}

class ModuleTrayAction {
  const ModuleTrayAction({
    required this.id,
    required this.title,
    required this.command,
  });
  final String id;
  final String title;

  /// Takes no arguments.
  final String command;
}
