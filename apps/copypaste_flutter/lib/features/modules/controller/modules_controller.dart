import 'package:flutter/foundation.dart';
import 'package:pub_semver/pub_semver.dart';
import '../models/module_marketplace_models.dart';
import '../models/module_models.dart';
import '../repository/module_marketplace_repository.dart';
import '../repository/modules_repository.dart';
import 'module_form_draft.dart';

class ModulesController extends ChangeNotifier {
  ModulesController({
    required ModulesRepository repository,
    required ModuleMarketplaceRepository marketplace,
    ModuleInputPicker? inputPicker,
    Future<void> Function()? restart,
  }) : _repository = repository,
       _marketplace = marketplace,
       _inputPicker = inputPicker,
       _restart = restart;
  final ModulesRepository _repository;
  final ModuleMarketplaceRepository _marketplace;
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
  ModulesSection _section = ModulesSection.marketplace;
  ModulesLoadState _catalogState = ModulesLoadState.loading;
  List<MarketplaceModule> _catalog = const [];
  String? _catalogError;
  bool _catalogLoading = false;
  bool _catalogRequested = false;
  String _query = '';
  String? _activeModuleId;
  double? _downloadProgress;
  bool _installing = false;
  ModulesSection get section => _section;
  ModulesLoadState get catalogState => _catalogState;
  String? get catalogError => _catalogError;
  String? get activeModuleId => _activeModuleId;
  double? get downloadProgress => _downloadProgress;
  bool get installing => _installing;
  String get query => _query;
  List<MarketplaceModule> get catalog => List.unmodifiable(
    _catalog.where((module) => _matches(module.title, module.description)),
  );
  List<InstalledModule> get filteredModules => List.unmodifiable(
    _modules.where((module) => _matches(module.title, module.description)),
  );
  bool _matches(String title, String description) =>
      '$title $description'.toLowerCase().contains(_query.trim().toLowerCase());

  void selectSection(ModulesSection section) {
    if (_disposed || _section == section) return;
    _section = section;
    notifyListeners();
  }

  void search(String query) {
    if (_disposed || _query == query) return;
    _query = query;
    notifyListeners();
  }

  InstalledModule? installedModule(String id) {
    for (final module in _modules) {
      if (module.id == id) return module;
    }
    return null;
  }

  MarketplaceModule? updateFor(InstalledModule module) {
    for (final release in _catalog) {
      if (release.id == module.id &&
          release.version > Version.parse(module.version)) {
        return release;
      }
    }
    return null;
  }

  Future<void> ensureMarketplace() async {
    if (!_catalogRequested) await loadMarketplace();
  }

  Future<void> loadMarketplace() async {
    if (_disposed || _catalogLoading) return;
    _catalogLoading = true;
    _catalogRequested = true;
    _catalogState = ModulesLoadState.loading;
    _catalogError = null;
    notifyListeners();
    try {
      final catalog = await _marketplace.list();
      if (!_disposed) {
        _catalog = catalog;
        _catalogState = ModulesLoadState.ready;
      }
    } catch (error) {
      if (!_disposed) {
        _catalogState = ModulesLoadState.error;
        _catalogError = error is ModulesException
            ? error.message
            : 'The module marketplace could not be loaded.';
      }
    } finally {
      _catalogLoading = false;
      if (!_disposed) notifyListeners();
    }
  }

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

  Future<void> install(MarketplaceModule module) async {
    final installed = installedModule(module.id);
    if (_busy ||
        _disposed ||
        !_catalog.contains(module) ||
        (installed != null &&
            (installed.restartRequired ||
                Version.parse(installed.version) >= module.version))) {
      return;
    }
    _activeModuleId = module.id;
    _downloadProgress = 0;
    _installing = false;
    try {
      await _perform(() async {
        final package = await _marketplace.download(
          module,
          onProgress: (progress) {
            if (_disposed) return;
            final bounded = progress.clamp(0.0, 1.0);
            if ((_downloadProgress! * 100).floor() == (bounded * 100).floor()) {
              return;
            }
            _downloadProgress = bounded;
            notifyListeners();
          },
        );
        try {
          if (_disposed) return;
          _installing = true;
          notifyListeners();
          await _repository.install(package.path);
        } finally {
          await package.dispose();
        }
      });
    } finally {
      _activeModuleId = null;
      _downloadProgress = null;
      _installing = false;
      if (!_disposed) notifyListeners();
    }
  }

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
    _marketplace.dispose();
    super.dispose();
  }
}
