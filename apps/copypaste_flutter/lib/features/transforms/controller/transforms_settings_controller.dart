import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import '../models/transform_models.dart';
import '../repository/transform_repository.dart';

enum TransformsSettingsStatus { idle, loading, ready, failed }

/// Settings → Transforms state. It loads when a settings view attaches and
/// drops the catalogue and its change subscription when the last one
/// detaches, so closed settings hold nothing.
class TransformsSettingsController extends ChangeNotifier {
  TransformsSettingsController({
    required TransformRepository repository,
    required this.platform,
    String? locale,
  }) : _repository = repository,
       _locale = locale;

  /// The runtime platform name per-app defaults are keyed by on this device.
  final String platform;

  /// The platform name of [target] in per-app default keys.
  static String platformName(TargetPlatform target) => switch (target) {
    TargetPlatform.macOS => 'macos',
    TargetPlatform.windows => 'windows',
    TargetPlatform.android => 'android',
    TargetPlatform.iOS => 'ios',
    TargetPlatform.fuchsia => 'fuchsia',
    TargetPlatform.linux => 'linux',
  };
  final TransformRepository _repository;
  final String? _locale;

  TransformsSettingsStatus _status = TransformsSettingsStatus.idle;
  List<TransformDefinition> _catalogue = const [];
  Map<String, TransformDefinition> _byId = const {};
  List<TransformPipeline> _pipelines = const [];
  TransformPreferences _preferences = const TransformPreferences();
  String? _error;
  bool _busy = false;
  int _attached = 0;
  int _generation = 0;
  StreamSubscription<void>? _changes;
  bool _disposed = false;

  TransformsSettingsStatus get status => _status;
  List<TransformDefinition> get catalogue => _catalogue;
  List<TransformPipeline> get pipelines => _pipelines;
  TransformPreferences get preferences => _preferences;
  String? get errorMessage => _error;
  bool get busy => _busy;

  TransformDefinition? definition(String id) => _byId[id];

  TransformPipeline? pipeline(String id) =>
      _pipelines.where((pipeline) => pipeline.id == id).firstOrNull;

  /// Pipelines that may run as a paste default: every step is known and
  /// deterministic.
  List<TransformPipeline> get defaultCandidates => [
    for (final pipeline in _pipelines)
      if (pipeline.steps.isNotEmpty &&
          pipeline.steps.every((step) {
            final definition = _byId[step.transformId];
            return definition != null &&
                definition.canBeDefault &&
                definition.deterministic;
          }))
        pipeline,
  ];

  bool pipelineUnavailable(TransformPipeline pipeline) => pipeline.steps.any(
    (step) =>
        _byId[step.transformId] == null ||
        step.version > _byId[step.transformId]!.version,
  );

  void attach() {
    if (_attached++ > 0) return;
    _changes = _repository.changes().listen((_) => unawaited(_reload()));
    unawaited(_reload());
  }

  void detach() {
    if (_attached == 0 || --_attached > 0) return;
    unawaited(_changes?.cancel());
    _changes = null;
    _generation++;
    _catalogue = const [];
    _byId = const {};
    _pipelines = const [];
    _status = TransformsSettingsStatus.idle;
  }

  Future<void> _reload() async {
    final generation = ++_generation;
    if (_status != TransformsSettingsStatus.ready) {
      _status = TransformsSettingsStatus.loading;
      notifyListeners();
    }
    try {
      final loaded = await Future.wait<Object>([
        if (_catalogue.isEmpty)
          _repository.catalogue(_locale ?? AppStrings.current.localeName)
        else
          Future.value(_catalogue),
        _repository.pipelines(),
        _repository.preferences(),
      ]);
      if (_disposed || generation != _generation) return;
      _catalogue = loaded[0] as List<TransformDefinition>;
      _byId = {for (final definition in _catalogue) definition.id: definition};
      _pipelines = loaded[1] as List<TransformPipeline>;
      _preferences = loaded[2] as TransformPreferences;
      _status = TransformsSettingsStatus.ready;
      _error = null;
    } on Object catch (error) {
      if (_disposed || generation != _generation) return;
      _status = TransformsSettingsStatus.failed;
      _error = _message(error);
    }
    notifyListeners();
  }

  Future<void> retry() => _reload();

  Future<bool> _update(Future<TransformPreferences> Function() change) =>
      _mutate(() async => _preferences = await change());

  Future<bool> _mutate(Future<void> Function() change) async {
    if (_busy) return false;
    _busy = true;
    _error = null;
    notifyListeners();
    try {
      await change();
      return true;
    } on Object catch (error) {
      _error = _message(error);
      return false;
    } finally {
      _busy = false;
      notifyListeners();
    }
  }

  static String _message(Object error) => error is TransformException
      ? error.message
      : AppStrings.current.transformsLoadFailed;

  Future<bool> _setFavorites(List<TransformFavorite> favorites) => _update(
    () => _repository.setPreference(TransformPreferenceKey.favorites, [
      for (final favorite in favorites) favorite.toJson(),
    ]),
  );

  Future<bool> addFavorite(TransformFavorite favorite) {
    if (_preferences.isFavorite(favorite)) return Future.value(true);
    return _setFavorites([..._preferences.favorites, favorite]);
  }

  Future<bool> removeFavorite(TransformFavorite favorite) => _setFavorites([
    ..._preferences.favorites.where((entry) => entry != favorite),
  ]);

  Future<bool> moveFavorite(int index, int delta) {
    final favorites = [..._preferences.favorites];
    final target = index + delta;
    if (index < 0 ||
        index >= favorites.length ||
        target < 0 ||
        target >= favorites.length) {
      return Future.value(false);
    }
    favorites.insert(target, favorites.removeAt(index));
    return _setFavorites(favorites);
  }

  /// A favorite's display name, or null when it no longer resolves.
  String? favoriteName(TransformFavorite favorite) => switch (favorite.kind) {
    TransformFavoriteKind.transform => _byId[favorite.id]?.name,
    TransformFavoriteKind.pipeline => pipeline(favorite.id)?.name,
  };

  bool pipelineNameTaken(String name, {String? exceptId}) => _pipelines.any(
    (pipeline) =>
        pipeline.id != exceptId &&
        pipeline.name.toLowerCase() == name.trim().toLowerCase(),
  );

  Future<bool> savePipeline(TransformPipelineDraft draft) => _mutate(() async {
    final saved = await _repository.savePipeline(draft);
    _pipelines = [
      for (final pipeline in _pipelines)
        if (pipeline.id != saved.id) pipeline,
      saved,
    ]..sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
  });

  Future<bool> duplicatePipeline(TransformPipeline pipeline) {
    var index = 2;
    String name;
    do {
      name = AppStrings.current.transformsPipelineCopyName(
        pipeline.name,
        index,
      );
      index++;
    } while (pipelineNameTaken(name));
    return savePipeline(
      TransformPipelineDraft(
        name: name,
        description: pipeline.description,
        steps: pipeline.steps,
      ),
    );
  }

  /// Deletes a pipeline; favorites and defaults that named it fall back to
  /// none where they are next used.
  Future<bool> deletePipeline(String id) => _mutate(() async {
    await _repository.deletePipeline(id);
    _pipelines = [..._pipelines.where((pipeline) => pipeline.id != id)];
  });

  Future<bool> setDefaultPipeline(String? id) => _update(
    () => _repository.setPreference(
      TransformPreferenceKey.defaultPastePipeline,
      id,
    ),
  );

  /// [pipelineId] null means "no default" for that application.
  Future<bool> setAppDefault(String identifier, String? pipelineId) {
    final trimmed = identifier.trim();
    if (trimmed.isEmpty) return Future.value(false);
    return _update(
      () => _repository.setPreference(
        TransformPreferenceKey.appDefault(platform, trimmed),
        {'pipeline_id': pipelineId},
      ),
    );
  }

  Future<bool> clearAppDefault(TransformAppDefault entry) => _update(
    () => _repository.setPreference(
      TransformPreferenceKey.appDefault(entry.platform, entry.identifier),
      null,
    ),
  );

  Future<bool> setRestoreClipboard(bool value) => _update(
    () => _repository.setPreference(
      TransformPreferenceKey.restoreClipboard,
      value,
    ),
  );

  Future<bool> setSuggestions(bool value) => _update(
    () => _repository.setPreference(TransformPreferenceKey.suggestions, value),
  );

  Future<bool> setDetector(TransformDetector detector, bool enabled) => _update(
    () => _repository.setMember(
      TransformSet.disabledDetectors,
      detector.wireName,
      present: !enabled,
    ),
  );

  Future<bool> setHidden(String transformId, bool hidden) => _update(
    () => _repository.setMember(
      TransformSet.hidden,
      transformId,
      present: hidden,
    ),
  );

  /// Adds a tracking parameter to remove or to keep; `name@domain` scopes
  /// it to one domain.
  Future<bool> setTrackingParameter(
    TransformSet set,
    String member, {
    required bool present,
  }) {
    final trimmed = member.trim();
    if (trimmed.isEmpty ||
        (set != TransformSet.trackingRemove &&
            set != TransformSet.trackingAllow)) {
      return Future.value(false);
    }
    return _update(() => _repository.setMember(set, trimmed, present: present));
  }

  @override
  void notifyListeners() {
    if (!_disposed) super.notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    unawaited(_changes?.cancel());
    super.dispose();
  }
}
