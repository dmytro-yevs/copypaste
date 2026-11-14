import 'package:flutter/foundation.dart';

import '../models/module_models.dart';
import '../models/module_view.dart';
import 'modules_controller.dart';

/// The view one module's settings show. A view result replaces it; after
/// any other result, the command that produced the view runs again.
class ModuleViewController extends ChangeNotifier {
  ModuleViewController({
    required ModulesController modules,
    required String moduleId,
  }) : _modules = modules,
       _moduleId = moduleId;

  final ModulesController _modules;
  final String _moduleId;
  ModuleView? _view;
  (String, Map<String, Object>)? _source;
  bool _disposed = false;

  ModuleView? get view => _view;

  /// Runs [command]; returns a result for the caller to show, or null when
  /// it failed or became the view.
  Future<ModuleResult?> run(
    String command,
    Map<String, Object> arguments,
  ) async {
    final result = await _modules.invoke(_moduleId, command, arguments);
    if (_disposed || result == null) return null;
    if (result.view case final view?) {
      _show(view, (command, Map.unmodifiable(arguments)));
      return null;
    }
    if (_source case (final command, final arguments)?) {
      final refreshed = await _modules.invoke(_moduleId, command, arguments);
      if (!_disposed && refreshed?.view != null) {
        _show(refreshed!.view!, _source!);
      }
    }
    return result;
  }

  void _show(ModuleView view, (String, Map<String, Object>) source) {
    _view = view;
    _source = source;
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
