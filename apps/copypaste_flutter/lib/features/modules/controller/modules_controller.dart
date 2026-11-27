import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:pub_semver/pub_semver.dart';

import '../../../l10n/l10n.dart';
import '../models/module_marketplace_models.dart';
import '../../../shared/error_text.dart';
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
    ModulePackagePicker? packagePicker,
    Future<void> Function()? restart,
    ModuleAccessRepository? access,
    ModuleDeviceCatalog? devices,
    Stream<void>? reconnects,
    List<String> Function()? preferredLanguages,
  }) : _repository = repository,
       _preferredLanguages = preferredLanguages ?? _systemLanguages,
       _marketplace = marketplace,
       _inputPicker = inputPicker,
       _packagePicker = packagePicker,
       _restart = restart,
       _access = access,
       _devices = devices {
    _reconnects = reconnects?.listen(
      (_) => unawaited(_applyLocale()),
      onError: (Object _) {},
    );
  }
  late final StreamSubscription<void>? _reconnects;
  final List<String> Function() _preferredLanguages;

  static List<String> _systemLanguages() => [
    for (final locale in PlatformDispatcher.instance.locales)
      locale.languageCode,
  ];
  final ModuleAccessRepository? _access;
  final ModuleDeviceCatalog? _devices;
  final ModulesRepository _repository;
  final ModuleMarketplaceRepository _marketplace;
  final ModuleInputPicker? _inputPicker;
  final ModulePackagePicker? _packagePicker;
  bool get canInstallFromFile => _packagePicker != null;
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
  String? _errorReason;
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

  /// Installed modules first, then installable ones, unavailable ones last;
  /// the catalog order holds within each group.
  List<MarketplaceModule> get catalog {
    int rank(MarketplaceModule module) => installedModule(module.id) != null
        ? 0
        : module.canInstall
        ? 1
        : 2;
    final matching = _catalog
        .where((module) => _matches(module.title, module.description))
        .toList();
    final order = {
      for (final (index, module) in matching.indexed) module: index,
    };
    matching.sort((a, b) {
      final byRank = rank(a).compareTo(rank(b));
      return byRank != 0 ? byRank : order[a]!.compareTo(order[b]!);
    });
    return List.unmodifiable(matching);
  }

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

  /// Commands the user runs from the module's settings; those the host
  /// fills from a selection or a form run only from the module's views.
  List<ModuleCommand> settingsCommands(InstalledModule module) =>
      List.unmodifiable(
        module.commands.where(
          (command) =>
              (module.id != 'copypaste.ocr' ||
                  command.id != 'recognize-image') &&
              !command.arguments.any((field) => field.kind.hostFilled),
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
        _catalogError = error is ModuleMarketplaceUnavailableException
            ? null
            : ErrorText.reason(error);
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

  /// Why the last operation failed, or null when no cause is known; the line
  /// under a title that already says a module operation failed.
  String? get errorReason => _errorReason;

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
        // Lost SMS access is reported, but the list stays usable so the
        // module's settings can set the access up again.
        _state = ModulesLoadState.ready;
        await _refreshAccess();
      }
    }, refresh: false);
    if (!_disposed && _state == ModulesLoadState.loading) {
      _state = ModulesLoadState.error;
      notifyListeners();
    }
    // The language resolved while the first list was in flight.
    if (!_disposed && _locale != listedLocale) unawaited(_applyLocale());
    // Choosing languages can download a search model, which must not hold
    // up startup; setPreferences reports its own failures.
    unawaited(_chooseSearchLanguages());
  }

  /// A search module needs languages before it can search; one installed
  /// without them gets the system languages it supports, else English.
  Future<void> _chooseSearchLanguages() async {
    for (final module in List.of(_modules)) {
      final field = module.searchLanguageField;
      if (_disposed || field == null || !module.enabled) continue;
      final chosen = module.preferences[field];
      if (chosen is List && chosen.isNotEmpty) continue;
      final options = [
        for (final spec in module.preferenceFields)
          if (spec.id == field)
            for (final option in spec.options) option.id,
      ];
      final preferred = <String>{
        for (final language in _preferredLanguages())
          if (options.contains(language)) language,
      }.toList();
      final languages =
          [
            preferred,
            preferred.take(1).toList(),
            if (options.contains('en')) ['en'],
            options.take(1).toList(),
          ].firstWhere(
            (candidate) =>
                ModuleSearchModel.forLanguages(
                  module.searchModels,
                  candidate,
                ) !=
                null,
            orElse: () => const [],
          );
      if (languages.isEmpty) continue;
      await setPreferences(module.id, {
        ...module.preferences,
        field: languages,
      });
    }
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
    if (installedPackage && !_disposed) await _chooseSearchLanguages();
  }

  /// Installs a package file the user chooses; the runtime verifies its
  /// signature like a marketplace download.
  Future<void> installFromFile() async {
    final picker = _packagePicker;
    if (picker == null) return;
    await _perform(() async {
      final package = await picker.choosePackage();
      if (package == null) return;
      try {
        await _repository.install(package.path);
      } finally {
        await package.dispose();
      }
      _section = ModulesSection.installed;
    });
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
    if (sms &&
        _access != null &&
        await _access.synchronize() != SmsMonitoringStatus.ready) {
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
    if (await _monitoringProblem() case final problem?) {
      throw ModulesException(problem);
    }
  }

  /// Applies the enabled SMS modules to native monitoring and explains why
  /// an enabled one is not monitored, if it is not.
  Future<String?> _monitoringProblem() async {
    if (_access == null) return null;
    final status = await _access.synchronize();
    if (!_modules.any(
      (module) =>
          module.enabled && module.events.contains(ModuleEventKind.smsReceived),
    )) {
      return null;
    }
    return switch (status) {
      SmsMonitoringStatus.ready => null,
      SmsMonitoringStatus.accessMissing =>
        AppStrings.current.modulesSmsMonitoringNeedsSetup,
      SmsMonitoringStatus.serviceRefused =>
        AppStrings.current.modulesSmsMonitoringFailed,
      SmsMonitoringStatus.unavailable =>
        AppStrings.current.modulesSmsMonitoringUnavailable,
    };
  }

  String? _monitoringNotice;

  /// A monitoring problem found after an operation succeeded. It does not
  /// fail that operation, so it is reported apart from [errorMessage].
  String? get monitoringNotice => _monitoringNotice;

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

  /// Runs a command for a module surface or view. Unlike [invoke] it neither
  /// waits for nor holds [busy], so surfaces query modules while other work
  /// runs; failures throw [ModulesException].
  Future<ModuleResult> request(
    String id,
    String command,
    Map<String, Object> arguments,
  ) => _repository.invoke(id, command, arguments);

  ModuleDrawerRequest? _drawerRequest;

  /// A command whose view the module's settings section opens in a drawer,
  /// such as a tray action's; the section takes it once it shows.
  ModuleDrawerRequest? get drawerRequest => _drawerRequest;

  void requestDrawer(ModuleDrawerRequest request) {
    if (_disposed) return;
    _drawerRequest = request;
    notifyListeners();
  }

  /// The pending request for [moduleId], which no other section sees again.
  ModuleDrawerRequest? takeDrawerRequest(String moduleId) {
    final request = _drawerRequest;
    if (request?.moduleId != moduleId) return null;
    _drawerRequest = null;
    return request;
  }

  /// Modules whose surfaces show: enabled, loaded and not restarting.
  List<InstalledModule> get activeModules =>
      List.unmodifiable(_modules.where((module) => module.active));

  Future<T?> _perform<T>(
    Future<T> Function() operation, {
    bool refresh = true,
  }) async {
    if (_busy || _disposed) return null;
    _busy = true;
    _error = null;
    _errorReason = null;
    _monitoringNotice = null;
    if (_operation?.failed ?? false) _operation = null;
    notifyListeners();
    T? result;
    try {
      result = await operation();
      if (refresh && !_disposed) {
        _modules = await _repository.list();
        _state = ModulesLoadState.ready;
        try {
          _monitoringNotice = await _monitoringProblem();
        } catch (error) {
          _monitoringNotice =
              ErrorText.reason(error) ??
              AppStrings.current.modulesSmsMonitoringUnavailable;
        }
      }
    } catch (error) {
      if (!_disposed) {
        _errorReason = ErrorText.reason(error);
        _error = _errorReason ?? AppStrings.current.modulesOperationIncomplete;
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

class ModuleDrawerRequest {
  const ModuleDrawerRequest({
    required this.moduleId,
    required this.command,
    required this.title,
  });
  final String moduleId;
  final String command;
  final String title;
}
