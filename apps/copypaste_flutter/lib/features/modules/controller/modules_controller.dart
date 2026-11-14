import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:pub_semver/pub_semver.dart';

import '../../../l10n/l10n.dart';
import '../models/module_marketplace_models.dart';
import '../models/module_models.dart';
import '../models/module_operation.dart';
import '../repository/module_marketplace_repository.dart';
import '../repository/modules_repository.dart';
import '../repository/module_access_repository.dart';
import '../../../shared/forms/dynamic_form_draft.dart';
import 'sms_access_setup_controller.dart';

class ModulesController extends ChangeNotifier {
  ModulesController({
    required ModulesRepository repository,
    required ModuleMarketplaceRepository marketplace,
    ModuleInputPicker? inputPicker,
    Future<void> Function()? restart,
    ModuleAccessRepository? access,
    ModuleDeviceCatalog? devices,
    Stream<void>? reconnects,
  }) : _repository = repository,
       _marketplace = marketplace,
       _inputPicker = inputPicker,
       _restart = restart,
       _access = access,
       _devices = devices {
    _reconnects = reconnects?.listen(
      (_) => unawaited(_applyLocale()),
      onError: (Object _) {},
    );
  }
  late final StreamSubscription<void>? _reconnects;
  final ModuleAccessRepository? _access;
  final ModuleDeviceCatalog? _devices;
  final ModulesRepository _repository;
  final ModuleMarketplaceRepository _marketplace;
  final ModuleInputPicker? _inputPicker;
  final Future<void> Function()? _restart;
  bool get canRestart => _restart != null;
  Future<void> restartApplication() => _perform(() async {
    await _restart?.call();
  }, refresh: false);
  DynamicFormDraft form(
    List<ModuleField> fields,
    Map<String, Object> initial, {
    InstalledModule? module,
  }) => DynamicFormDraft(
    fields: fields,
    initial: initial,
    picker: _inputPicker,
    accepts: module?.acceptsPreferences,
  );
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
  ModuleOperation? _operation;
  bool _configuringModel = false;
  bool get configuringModel => _configuringModel;
  ModulesSection get section => _section;
  ModulesLoadState get catalogState => _catalogState;
  String? get catalogError => _catalogError;
  String? get activeModuleId => _activeModuleId;

  /// The running install or update of [id], or its last failed attempt.
  ModuleOperation? operationFor(String id) =>
      _operation?.moduleId == id ? _operation : null;
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

  MarketplaceModule? marketplaceModule(String id) {
    for (final module in _catalog) {
      if (module.id == id) return module;
    }
    return null;
  }

  List<ModuleCommand> settingsCommands(InstalledModule module) =>
      List.unmodifiable(
        module.commands.where(
          (command) =>
              module.id != 'copypaste.ocr' || command.id != 'recognize-image',
        ),
      );

  MarketplaceModule? updateFor(InstalledModule module) {
    for (final release in _catalog) {
      if (release.canInstall &&
          release.id == module.id &&
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
            : AppStrings.current.modulesMarketplaceLoadFailed;
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
    String? listedLocale;
    await _perform(() async {
      listedLocale = _locale;
      await _localeSent;
      final modules = await _repository.list();
      if (!_disposed) {
        _modules = modules;
        await _refreshAccess();
        _state = ModulesLoadState.ready;
      }
    }, refresh: false);
    if (!_disposed && _state == ModulesLoadState.loading) {
      _state = ModulesLoadState.error;
      notifyListeners();
    }
    // The language resolved while the first list was in flight.
    if (!_disposed && _locale != listedLocale) unawaited(_applyLocale());
  }

  Future<void> install(MarketplaceModule module) async {
    final installed = installedModule(module.id);
    if (_busy ||
        _disposed ||
        !module.canInstall ||
        !_catalog.contains(module) ||
        (installed != null &&
            (installed.restartRequired ||
                Version.parse(installed.version) >= module.version))) {
      return;
    }
    _activeModuleId = module.id;
    var operation = ModuleOperation.started(
      moduleId: module.id,
      kind: installed == null
          ? ModuleOperationKind.install
          : ModuleOperationKind.update,
      totalBytes: module.artifact!.sizeBytes,
    );
    _operation = operation;
    var installedPackage = false;
    try {
      await _perform(() async {
        final package = await _marketplace.download(
          module,
          onProgress: (progress) {
            if (_disposed) return;
            final next = operation.downloadedTo(progress);
            if (identical(next, operation)) return;
            _operation = operation = next;
            notifyListeners();
          },
        );
        try {
          if (_disposed) return;
          _operation = operation = operation.installing();
          notifyListeners();
          await _repository.install(package.path);
          installedPackage = true;
        } finally {
          await package.dispose();
        }
      });
    } finally {
      _activeModuleId = null;
      // A failure after activation is not a failed install phase.
      _operation = _error != null && !installedPackage && !_disposed
          ? operation.failedInPhase()
          : null;
      if (!_disposed) notifyListeners();
    }
  }

  /// Enabling a module that declares capabilities needs them [confirmed].
  Future<bool> setEnabled(
    String id,
    bool enabled, {
    List<ModuleCapability> confirmed = const [],
  }) async {
    final done = await _perform(() async {
      await _setEnabled(id, enabled, confirmed);
      return true;
    });
    return done == true && _error == null;
  }

  Future<void> _setEnabled(
    String id,
    bool enabled,
    List<ModuleCapability> confirmed,
  ) async {
    final sms =
        installedModule(id)?.events.contains(ModuleEventKind.smsReceived) ??
        false;
    if (sms && enabled) {
      final state = await _access?.smsState();
      if (state?.granted != true) {
        throw ModulesException(AppStrings.current.modulesSmsSetupRequired);
      }
    }
    await _repository.setEnabled(
      id,
      enabled,
      capabilities: enabled ? confirmed : const [],
    );
    if (sms && _access != null && !await _access.synchronize()) {
      if (enabled) {
        try {
          await _repository.setEnabled(id, false);
        } catch (_) {
          bool? persistedEnabled;
          try {
            final modules = await _repository.list();
            persistedEnabled = modules
                .where((module) => module.id == id)
                .firstOrNull
                ?.enabled;
          } catch (_) {
            // _perform refreshes again before it presents this error.
          }
          throw ModulesException(switch (persistedEnabled) {
            true => AppStrings.current.modulesSmsMonitoringStillEnabled,
            false => AppStrings.current.modulesSmsMonitoringDisabled,
            null => AppStrings.current.modulesSmsMonitoringUnconfirmed,
          });
        }
      }
      throw ModulesException(AppStrings.current.modulesSmsMonitoringFailed);
    }
  }

  String? _locale;
  Future<void>? _localeSent;

  /// Reports the app language, then rereads the summaries it localizes.
  /// The runtime may keep it in memory only, so it is sent again on every
  /// reconnect.
  void useLocale(String locale) {
    if (_disposed || _locale == locale) return;
    _locale = locale;
    unawaited(_applyLocale());
  }

  Future<void> _applyLocale() async {
    final locale = _locale;
    if (_disposed || locale == null) return;
    final sent = _localeSent = _repository.setLocale(locale).catchError((
      Object _,
    ) {
      // Summaries keep their English strings.
    });
    await sent;
    if (_disposed || _locale != locale || _state != ModulesLoadState.ready) {
      return;
    }
    try {
      final modules = await _repository.list();
      if (_disposed || _locale != locale || _busy) return;
      _modules = modules;
      notifyListeners();
    } catch (_) {
      // Keep the last known list.
    }
  }

  /// Named devices for device-set fields; none when they cannot be read.
  Future<List<ModuleChoice>> deviceChoices() async {
    try {
      return await _devices?.devices() ?? const [];
    } catch (_) {
      return const [];
    }
  }

  bool get canSetUpSmsAccess => _access != null;

  SmsAccessSetupController smsAccessSetup() {
    final access = _access;
    if (access == null) {
      throw ModulesException(AppStrings.current.modulesSmsAndroidOnly);
    }
    return SmsAccessSetupController(access: access);
  }

  Future<void> refreshSmsAccess() => _perform(_refreshAccess, refresh: false);

  Future<void> _refreshAccess() async {
    if (_access == null) return;
    if (!await _access.synchronize() &&
        _modules.any(
          (module) =>
              module.enabled &&
              module.events.contains(ModuleEventKind.smsReceived),
        )) {
      throw ModulesException(AppStrings.current.modulesSmsMonitoringNeedsSetup);
    }
  }

  Future<void> setPreferences(String id, Map<String, Object> values) async {
    if (_busy || _disposed) return;
    final module = installedModule(id);
    final languages = values[module?.searchLanguageField];
    final model = languages is List
        ? ModuleSearchModel.forLanguages(
            module?.searchModels ?? const [],
            languages.cast<String>(),
          )
        : null;
    _configuringModel = model != null && !model.available;
    _activeModuleId = _configuringModel ? id : null;
    try {
      await _perform(() => _repository.setPreferences(id, values));
    } finally {
      _configuringModel = false;
      _activeModuleId = null;
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> remove(String id) async {
    final removed = await _perform(() async {
      await _repository.remove(id);
      return true;
    });
    return removed == true && _error == null;
  }

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
    if (_operation?.failed ?? false) _operation = null;
    notifyListeners();
    T? result;
    try {
      result = await operation();
      if (refresh && !_disposed) {
        _modules = await _repository.list();
        await _refreshAccess();
        _state = ModulesLoadState.ready;
      }
    } catch (error) {
      if (!_disposed) {
        _error = error is ModulesException
            ? error.message
            : AppStrings.current.modulesOperationIncomplete;
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
    unawaited(_reconnects?.cancel());
    _marketplace.dispose();
    super.dispose();
  }
}
