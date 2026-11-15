import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import '../../../shared/error_text.dart';
import '../models/module_models.dart';
import '../models/module_view.dart';
import 'modules_controller.dart';

/// The view a module surface shows. A view result replaces it; after any
/// other result, the command that produced the view runs again unless the
/// view is [transient], such as a drawer that closes instead.
class ModuleViewController extends ChangeNotifier {
  ModuleViewController({
    required ModulesController modules,
    required this.moduleId,
    ModuleView? view,
    (String, Map<String, Object>)? source,
    this.transient = false,
  }) : _modules = modules,
       _view = view,
       _source = source;

  final ModulesController _modules;
  final String moduleId;
  final bool transient;
  ModuleView? _view;
  (String, Map<String, Object>)? _source;
  int _generation = 0;
  int _running = 0;
  String? _error;
  bool _disposed = false;

  ModulesController get modules => _modules;
  ModuleView? get view => _view;
  bool get busy => _running > 0;
  String? get errorMessage => _error;

  /// Shows the view [command] returns.
  Future<void> open(
    String command, [
    Map<String, Object> arguments = const {},
  ]) async {
    await run(command, arguments);
  }

  /// Runs the command that produced the current view again.
  Future<void> refresh() async {
    if (_source case (final command, final arguments)?) {
      await _request(command, arguments, show: true);
    }
  }

  /// Runs [command]; returns a result for the caller to present, or null
  /// when it failed or became the view. With [show] off a view result is
  /// returned instead, for example to open it in a drawer.
  Future<ModuleResult?> run(
    String command,
    Map<String, Object> arguments, {
    bool show = true,
  }) async {
    final result = await _request(command, arguments, show: show);
    if (result == null || result.view != null || transient) return result;
    await refresh();
    return result;
  }

  /// Runs [command] without touching the view, such as a form preview.
  Future<ModuleResult?> peek(String command, Map<String, Object> arguments) =>
      _request(command, arguments, show: false, report: false);

  Future<ModuleResult?> _request(
    String command,
    Map<String, Object> arguments, {
    required bool show,
    bool report = true,
  }) async {
    if (_disposed) return null;
    final generation = show ? ++_generation : _generation;
    _running += 1;
    if (report) _error = null;
    notifyListeners();
    try {
      final result = await _modules.request(moduleId, command, arguments);
      if (_disposed) return null;
      if (result.view case final view? when show) {
        // A later request, such as the next search edit, owns the view.
        if (generation == _generation) {
          _view = view;
          _source = (command, Map.unmodifiable(arguments));
        }
        return null;
      }
      return result;
    } catch (error) {
      if (!_disposed && report) {
        _error =
            ErrorText.reason(error) ??
            AppStrings.current.modulesOperationIncomplete;
      }
      return null;
    } finally {
      _running -= 1;
      if (!_disposed) notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
