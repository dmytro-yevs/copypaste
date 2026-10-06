import 'package:flutter/foundation.dart';
import '../models/module_models.dart';
import '../repository/modules_repository.dart';
import 'module_form_draft.dart';

class ModulesController extends ChangeNotifier {
  ModulesController({
    required ModulesRepository repository,
    required ModulePackagePicker picker,
    ModuleInputPicker? inputPicker,
    Future<void> Function()? restart,
  }) : _repository = repository,
       _picker = picker,
       _inputPicker = inputPicker,
       _restart = restart;
  final ModulesRepository _repository;
  final ModulePackagePicker _picker;
  final ModuleInputPicker? _inputPicker;
  final Future<void> Function()? _restart;
  bool get canRestart => _restart != null;
  Future<void> restartApplication() => _perform(() async {
    await _restart?.call();
  }, refresh: false);
  ModuleFormDraft form(List<ModuleField> fields, Map<String, Object> initial) =>
      ModuleFormDraft(fields: fields, initial: initial, picker: _inputPicker);
  ModulesLoadState _state = ModulesLoadState.loading;
  List<InstalledModule> _modules = const [];
  bool _busy = false;
  bool _disposed = false;
  String? _error;
  ModulesLoadState get state => _state;
  List<InstalledModule> get modules => _modules;
  bool get busy => _busy;
  String? get errorMessage => _error;

  Future<void> initialize() async {
    if (_disposed || _busy) return;
    _state = ModulesLoadState.loading;
    await _perform(() async {
      final modules = await _repository.list();
      if (!_disposed) {
        _modules = modules;
        _state = ModulesLoadState.ready;
      }
    }, refresh: false);
    if (!_disposed && _state == ModulesLoadState.loading) {
      _state = ModulesLoadState.error;
      notifyListeners();
    }
  }

  Future<void> install() => _perform(() async {
    final package = await _picker.choose();
    if (package == null) return;
    try {
      await _repository.install(package.path);
    } finally {
      await package.dispose();
    }
  });
  Future<void> setEnabled(String id, bool enabled) =>
      _perform(() => _repository.setEnabled(id, enabled));
  Future<void> setPreferences(String id, Map<String, Object> values) =>
      _perform(() => _repository.setPreferences(id, values));
  Future<void> remove(String id) => _perform(() => _repository.remove(id));
  Future<ModuleResult?> invoke(
    String id,
    String command,
    Map<String, Object> arguments,
  ) => _perform(
    () => _repository.invoke(id, command, arguments),
    refresh: false,
  );

  Future<T?> _perform<T>(
    Future<T> Function() operation, {
    bool refresh = true,
  }) async {
    if (_busy || _disposed) return null;
    _busy = true;
    _error = null;
    notifyListeners();
    T? result;
    try {
      result = await operation();
      if (refresh && !_disposed) {
        _modules = await _repository.list();
        _state = ModulesLoadState.ready;
      }
    } catch (error) {
      if (!_disposed) {
        _error = error is ModulesException
            ? error.message
            : 'The module operation could not be completed.';
      }
      if (refresh && !_disposed) {
        // A failed cleanup can still persist a removal state. Read that state
        // without replaying the mutation, so the user can finish removal.
        try {
          _modules = await _repository.list();
        } catch (_) {
          /* Keep the last known list. */
        }
      }
    } finally {
      if (!_disposed) {
        _busy = false;
        notifyListeners();
      }
    }
    return result;
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
