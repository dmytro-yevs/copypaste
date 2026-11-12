import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import '../../../platform/desktop/quick_paste_host.dart';
import '../models/transform_models.dart';
import '../presentation/transform_messages.dart';
import '../repository/transform_recents_store.dart';
import '../repository/transform_repository.dart';
import 'transform_entries.dart';

enum TransformPaletteStatus { loading, ready, notText, empty, failed }

enum TransformActionStatus {
  completed,

  /// A paste could not reach the target; the result is on the clipboard.
  onClipboard,
  failed,
}

class TransformActionOutcome {
  const TransformActionOutcome(this.action, this.status, {this.paste});

  final TransformRunAction action;
  final TransformActionStatus status;
  final PasteOutcome? paste;
}

/// One transform palette session: its source, catalogue, chain, preview and
/// actions. The catalogue lives only as long as the session.
///
/// The chain is the committed steps plus the highlighted candidate. Every
/// change schedules one debounced preview; a newer preview supersedes the
/// older one, and closing the session cancels whatever is still running.
class TransformController extends ChangeNotifier {
  TransformController({
    required TransformRepository repository,
    required this.source,
    TransformRecentsStore? recents,
    Future<PasteOutcome> Function()? pasteForeground,
    String? locale,
    this.debounce = const Duration(milliseconds: 120),
    this.spinnerDelay = const Duration(milliseconds: 150),
    this.cancelDelay = const Duration(seconds: 1),
    this.restoreDelay = const Duration(milliseconds: 500),
  }) : _repository = repository,
       _recentsStore = recents ?? MemoryTransformRecentsStore(),
       _pasteForeground = pasteForeground,
       _locale = locale;

  static const maxRecents = 8;
  static const maxFavoriteShortcuts = 9;
  static const maxPipelineName = 80;

  final TransformSource source;
  final Duration debounce;
  final Duration spinnerDelay;
  final Duration cancelDelay;
  final Duration restoreDelay;
  final TransformRepository _repository;
  final TransformRecentsStore _recentsStore;
  final Future<PasteOutcome> Function()? _pasteForeground;
  final String? _locale;

  TransformPaletteStatus _status = TransformPaletteStatus.loading;
  String? _loadError;
  List<TransformDefinition> _catalogue = const [];
  Map<String, TransformDefinition> _byId = const {};
  List<TransformPipeline> _pipelines = const [];
  TransformPreferences _preferences = const TransformPreferences();
  List<TransformRecent> _recents = const [];
  List<String> _suggestions = const [];

  final List<TransformStep> _steps = [];
  TransformEntry? _candidate;
  Map<String, Object?>? _candidateParams;
  int? _selectedStep;
  String? _query;

  TransformPreview? _preview;
  String? _previewChain;
  String? _previewError;
  bool _previewPending = false;
  bool _showSpinner = false;
  bool _showCancel = false;
  bool _revealed = false;
  Timer? _debounceTimer;
  Timer? _spinnerTimer;
  Timer? _cancelTimer;
  String? _inFlight;
  int _sequence = 0;

  TransformRunAction? _running;
  String? _actionError;
  bool _disposed = false;

  TransformPaletteStatus get status => _status;
  String? get loadError => _loadError;
  List<TransformDefinition> get catalogue => _catalogue;
  List<TransformPipeline> get pipelines => _pipelines;
  TransformPreferences get preferences => _preferences;
  List<TransformRecent> get recents => _recents;
  List<TransformStep> get steps => List.unmodifiable(_steps);
  TransformEntry? get candidate => _candidate;
  int? get selectedStep => _selectedStep;
  String? get query => _query;
  TransformPreview? get preview => _preview;
  String? get previewError => _previewError;
  bool get previewPending => _previewPending;
  bool get showSpinner => _showSpinner;
  bool get showCancel => _showCancel;
  TransformRunAction? get running => _running;
  String? get actionError => _actionError;
  bool get canPasteTarget => _pasteForeground != null;
  TransformRunAction get primaryAction =>
      canPasteTarget ? TransformRunAction.paste : TransformRunAction.copy;

  TransformDefinition? definition(String id) => _byId[id];

  /// The committed steps followed by the highlighted candidate's steps.
  List<TransformStep> get chain => [..._steps, ..._candidateSteps()];

  int get committedCount => _steps.length;

  /// The preview belongs to the chain as it is now.
  bool get previewCurrent =>
      _preview != null && _previewChain == _chainKey(chain);

  /// The output of the selected step, or of the whole chain.
  TransformStepPreview? get shownStep {
    final preview = _preview;
    if (preview == null || preview.steps.isEmpty) return null;
    final index = _selectedStep;
    if (index != null && index < preview.steps.length) {
      return preview.steps[index];
    }
    return preview.steps.last;
  }

  bool get masked => (_preview?.sensitive ?? false) && !_revealed;

  bool get _hasKnownFailure =>
      (previewCurrent && _preview!.failure != null) || _previewError != null;

  bool get _chainUnavailable => chain.any(
    (step) =>
        _byId[step.transformId] == null ||
        step.version > _byId[step.transformId]!.version,
  );

  bool get canRun =>
      _status == TransformPaletteStatus.ready &&
      _running == null &&
      chain.isNotEmpty &&
      !_chainUnavailable &&
      !_hasKnownFailure;

  bool get canPaste => canPasteTarget && canRun;
  bool get canCopy => canRun;

  /// Saving a result equal to its input would only duplicate the clip.
  bool get canSave => canRun && !(previewCurrent && _preview!.noChanges);

  bool get canPasteOriginal =>
      canPasteTarget &&
      _running == null &&
      _status == TransformPaletteStatus.ready &&
      source is! TransformTextSource;

  bool get canSavePipeline =>
      _status == TransformPaletteStatus.ready &&
      chain.isNotEmpty &&
      chain.length <= TransformLimits.maxSteps;

  /// The failed step offers hex output instead of binary text.
  bool get canUseHexOutput {
    final preview = _preview;
    final index = preview?.failedStep;
    if (!previewCurrent || index == null) return false;
    if (preview!.steps[index].failure?.code != 'binary_data') return false;
    return _hexParam(chain[index]) != null;
  }

  /// Favorites in their saved order, resolved to palette rows.
  List<TransformEntry> get favoriteEntries => [
    for (final favorite in _preferences.favorites) ?_favoriteEntry(favorite),
  ];

  Future<void> open() async {
    if (source case TransformClipSource(text: false)) {
      _status = TransformPaletteStatus.notText;
      notifyListeners();
      return;
    }
    try {
      final locale = _locale ?? AppStrings.current.localeName;
      final loaded = await Future.wait<Object>([
        _repository.catalogue(locale),
        _repository.pipelines(),
        _repository.preferences(),
        _recentsStore.read(),
      ]);
      if (_disposed) return;
      _catalogue = loaded[0] as List<TransformDefinition>;
      _byId = {for (final definition in _catalogue) definition.id: definition};
      _pipelines = loaded[1] as List<TransformPipeline>;
      _preferences = loaded[2] as TransformPreferences;
      _recents = loaded[3] as List<TransformRecent>;
      _status = TransformPaletteStatus.ready;
    } on Object catch (error) {
      if (_disposed) return;
      _status = TransformPaletteStatus.failed;
      _loadError = error is TransformException
          ? error.message
          : AppStrings.current.transformsLoadFailed;
      notifyListeners();
      return;
    }
    notifyListeners();
    unawaited(_loadSuggestions());
  }

  Future<void> _loadSuggestions() async {
    if (!_preferences.suggestionsEnabled) return;
    try {
      final suggestions = await _repository.suggest(source);
      if (_disposed) return;
      _suggestions = suggestions;
      notifyListeners();
    } on TransformException catch (error) {
      if (!_disposed) _sourceFailed(error);
    }
  }

  /// Palette rows for [query]: grouped while it is empty, ranked otherwise.
  List<TransformEntryGroup> groups([String? query]) {
    final visible = [
      for (final definition in _catalogue)
        if (!_preferences.hidden.contains(definition.id))
          TransformEntry.transform(definition),
    ];
    final pipelineEntries = [
      for (final pipeline in _pipelines) _pipelineEntry(pipeline),
    ];
    final text = query?.trim() ?? '';
    if (text.isNotEmpty) {
      final strings = AppStrings.current;
      final ranked = rankTransformEntries(
        [...pipelineEntries, ...visible],
        text,
        entryName,
        (entry) => switch (entry.kind) {
          TransformEntryKind.transform => [
            strings.transformCategory(entry.definition!.category),
            ...entry.definition!.keywords,
            entry.definition!.description,
          ].join(' '),
          _ => entry.pipeline?.description ?? '',
        },
      );
      return [
        if (ranked.isNotEmpty)
          TransformEntryGroup(TransformEntryGroupKind.results, ranked),
      ];
    }
    final suggested = [
      for (final id in _suggestions)
        if (_byId[id] case final definition?
            when !_preferences.hidden.contains(id))
          TransformEntry.transform(definition),
    ];
    final recent = [
      for (final recent in _recents.take(3)) TransformEntry.recent(recent),
    ];
    final favorites = favoriteEntries;
    return [
      if (suggested.isNotEmpty)
        TransformEntryGroup(TransformEntryGroupKind.suggested, suggested),
      if (recent.isNotEmpty)
        TransformEntryGroup(TransformEntryGroupKind.recent, recent),
      if (favorites.isNotEmpty)
        TransformEntryGroup(TransformEntryGroupKind.favorites, favorites),
      if (pipelineEntries.isNotEmpty)
        TransformEntryGroup(TransformEntryGroupKind.pipelines, pipelineEntries),
      for (final category in TransformCategory.values)
        if (visible.where((e) => e.definition!.category == category).toList()
            case final entries when entries.isNotEmpty)
          TransformEntryGroup(
            TransformEntryGroupKind.category,
            entries,
            category: category,
          ),
    ];
  }

  String entryName(TransformEntry entry) => switch (entry.kind) {
    TransformEntryKind.transform => entry.definition!.name,
    TransformEntryKind.pipeline => entry.pipeline!.name,
    TransformEntryKind.recent => _recentName(entry.recent!),
  };

  String _recentName(TransformRecent recent) {
    if (recent.pipelineId case final id?) {
      return _pipelines.where((p) => p.id == id).firstOrNull?.name ??
          AppStrings.current.transformsRemovedPipeline;
    }
    return [
      for (final step in recent.steps)
        _byId[step.transformId]?.name ?? step.transformId,
    ].join(' → ');
  }

  /// Remembers the filter text, so Backspace on an empty filter can remove
  /// the last step. It does not notify: the list renders the query itself.
  void setQuery(String? query) => _query = query;

  /// The list highlights [entry]; the preview follows it.
  void highlight(TransformEntry? entry) {
    if (_disposed || entry == _candidate) return;
    _candidate = entry;
    _candidateParams = null;
    _selectedStep = null;
    _actionError = null;
    notifyListeners();
    _schedulePreview();
  }

  /// Tab: keeps the candidate as a step, so the next highlight adds to it.
  bool commitCandidate() {
    final added = _candidateSteps();
    if (added.isEmpty ||
        _steps.length + added.length > TransformLimits.maxSteps) {
      return false;
    }
    _steps.addAll(added);
    _candidate = null;
    _candidateParams = null;
    _selectedStep = null;
    notifyListeners();
    _schedulePreview();
    return true;
  }

  bool removeLastStep() {
    if (_steps.isEmpty) return false;
    removeStep(_steps.length - 1);
    return true;
  }

  void removeStep(int index) {
    if (index < 0 || index >= _steps.length) return;
    _steps.removeAt(index);
    _selectedStep = null;
    notifyListeners();
    _schedulePreview();
  }

  void setStepEnabled(int index, bool enabled) {
    if (index < 0 || index >= _steps.length) return;
    _steps[index] = _steps[index].copyWith(enabled: enabled);
    notifyListeners();
    _schedulePreview();
  }

  /// Moves a committed step one place earlier or later.
  void moveStep(int index, int delta) {
    final target = index + delta;
    if (index < 0 ||
        index >= _steps.length ||
        target < 0 ||
        target >= _steps.length) {
      return;
    }
    final step = _steps.removeAt(index);
    _steps.insert(target, step);
    if (_selectedStep == index) _selectedStep = target;
    notifyListeners();
    _schedulePreview();
  }

  /// Shows the data after step [index], or after the whole chain for null.
  void selectStep(int? index) {
    if (index != null && (index < 0 || index >= chain.length)) return;
    if (_selectedStep == index) return;
    _selectedStep = index;
    notifyListeners();
  }

  /// The step whose parameters the pane edits: the selected one, else the
  /// last of the chain.
  int? get paramStepIndex {
    final length = chain.length;
    if (length == 0) return null;
    return _selectedStep ?? length - 1;
  }

  /// Committed steps and a single-transform candidate are editable; a
  /// pipeline candidate runs as saved.
  bool stepEditable(int index) =>
      index < _steps.length ||
      (index == _steps.length &&
          _candidate?.kind == TransformEntryKind.transform);

  Object? paramValue(int index, TransformParam param) {
    final step = chain[index];
    return step.params.containsKey(param.key)
        ? step.params[param.key]
        : param.defaultValue;
  }

  void setParam(int index, TransformParam param, Object? value) {
    if (!stepEditable(index) || !param.accepts(value)) return;
    if (index < _steps.length) {
      final step = _steps[index];
      _steps[index] = step.copyWith(params: {...step.params, param.key: value});
    } else {
      _candidateParams = {..._candidateStepParams(), param.key: value};
    }
    notifyListeners();
    _schedulePreview();
  }

  void useHexOutput() {
    final index = _preview?.failedStep;
    if (!canUseHexOutput || index == null) return;
    setParam(index, _hexParam(chain[index])!, 'hex');
  }

  TransformParam? _hexParam(TransformStep step) => _byId[step.transformId]
      ?.params
      .where(
        (param) =>
            param.key == 'output' &&
            param.options.any((option) => option.value == 'hex'),
      )
      .firstOrNull;

  void reveal() {
    if (_revealed) return;
    _revealed = true;
    notifyListeners();
  }

  /// Stops the running preview; its pane keeps the last result.
  void cancelPreview() {
    final request = _inFlight;
    _debounceTimer?.cancel();
    _clearProgress();
    _previewPending = false;
    _inFlight = null;
    _sequence++;
    if (request != null) unawaited(_cancelRequest(request));
    if (!_disposed) notifyListeners();
  }

  bool isFavorite(TransformEntry entry) =>
      entry.favorite != null && _preferences.isFavorite(entry.favorite!);

  Future<bool> toggleFavorite(TransformEntry entry) async {
    final favorite = entry.favorite;
    if (favorite == null) return false;
    final next = isFavorite(entry)
        ? [..._preferences.favorites.where((f) => f != favorite)]
        : [..._preferences.favorites, favorite];
    try {
      _preferences = await _repository.setPreference(
        TransformPreferenceKey.favorites,
        [for (final f in next) f.toJson()],
      );
      if (!_disposed) notifyListeners();
      return true;
    } on TransformException {
      return false;
    }
  }

  /// Saves the chain as a pipeline named [name]; null when the name is
  /// taken or the runtime refuses it.
  Future<TransformPipeline?> savePipeline(
    String name, {
    String? description,
  }) async {
    final trimmed = name.trim();
    if (!canSavePipeline ||
        trimmed.isEmpty ||
        trimmed.length > maxPipelineName ||
        pipelineNameTaken(trimmed)) {
      return null;
    }
    try {
      final saved = await _repository.savePipeline(
        TransformPipelineDraft(
          name: trimmed,
          description: description?.trim().isEmpty ?? true
              ? null
              : description!.trim(),
          steps: chain,
        ),
      );
      if (_disposed) return saved;
      _pipelines = [..._pipelines, saved];
      notifyListeners();
      return saved;
    } on TransformException catch (error) {
      if (!_disposed) {
        _actionError = error.message;
        notifyListeners();
      }
      return null;
    }
  }

  bool pipelineNameTaken(String name) => _pipelines.any(
    (pipeline) => pipeline.name.toLowerCase() == name.trim().toLowerCase(),
  );

  Future<TransformActionOutcome> paste() => _run(TransformRunAction.paste);

  Future<TransformActionOutcome> copy() => _run(TransformRunAction.copy);

  Future<TransformActionOutcome> saveNewClip() =>
      _run(TransformRunAction.saveNewClip);

  Future<TransformActionOutcome> runPrimary() => _run(primaryAction);

  /// Pastes the source as itself, bypassing any default transform.
  Future<TransformActionOutcome> pasteOriginal() {
    if (!canPasteOriginal) {
      return Future.value(
        const TransformActionOutcome(
          TransformRunAction.paste,
          TransformActionStatus.failed,
        ),
      );
    }
    return _run(TransformRunAction.paste, original: true);
  }

  /// Re-runs the last chain with its parameters.
  Future<TransformActionOutcome?> repeatLast() async {
    final last = _recents.firstOrNull;
    if (last == null || _status != TransformPaletteStatus.ready) return null;
    _applyChain(TransformEntry.recent(last));
    return _run(primaryAction);
  }

  /// Runs the favorite at [index] (0-based) with the primary action.
  Future<TransformActionOutcome?> runFavorite(int index) async {
    final favorites = favoriteEntries;
    if (index < 0 ||
        index >= favorites.length ||
        favorites[index].unavailable ||
        _status != TransformPaletteStatus.ready) {
      return null;
    }
    _applyChain(favorites[index]);
    return _run(primaryAction);
  }

  void _applyChain(TransformEntry entry) {
    _steps.clear();
    _candidate = entry;
    _candidateParams = null;
    _selectedStep = null;
    notifyListeners();
    _schedulePreview();
  }

  Future<TransformActionOutcome> _run(
    TransformRunAction action, {
    bool original = false,
  }) async {
    final allowed = original
        ? canPasteOriginal
        : switch (action) {
            TransformRunAction.paste => canPaste,
            TransformRunAction.copy => canCopy,
            TransformRunAction.saveNewClip => canSave,
          };
    if (!allowed) {
      return TransformActionOutcome(action, TransformActionStatus.failed);
    }
    final chain = this.chain;
    final pipelineId = original ? null : _wholePipelineId();
    _running = action;
    _actionError = null;
    notifyListeners();
    try {
      final result = await _repository.run(
        source,
        steps: original || pipelineId != null ? const [] : chain,
        pipelineId: pipelineId,
        action: action,
        bypassDefault: original,
      );
      if (!original) unawaited(_remember(chain, pipelineId));
      if (action != TransformRunAction.paste) {
        return TransformActionOutcome(action, TransformActionStatus.completed);
      }
      final PasteOutcome pasted;
      try {
        pasted = await _pasteForeground!();
      } on Object {
        return TransformActionOutcome(
          action,
          TransformActionStatus.onClipboard,
          paste: PasteOutcome.unsupported,
        );
      }
      if (pasted != PasteOutcome.pasted) {
        return TransformActionOutcome(
          action,
          TransformActionStatus.onClipboard,
          paste: pasted,
        );
      }
      if (result.restoreToken case final token?) {
        unawaited(_restore(token));
      }
      return TransformActionOutcome(
        action,
        TransformActionStatus.completed,
        paste: pasted,
      );
    } on TransformException catch (error) {
      if (!_disposed) _actionError = error.message;
      return TransformActionOutcome(action, TransformActionStatus.failed);
    } finally {
      _running = null;
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> _restore(String token) async {
    try {
      await _repository.restoreClipboard(token, restoreDelay);
    } on TransformException {
      // The result stays on the clipboard, which is the safe fallback.
    }
  }

  /// A saved pipeline runs by id when the chain is exactly that pipeline,
  /// so the new clip records its name.
  String? _wholePipelineId() {
    if (_steps.isNotEmpty) return null;
    return switch (_candidate) {
      TransformEntry(kind: TransformEntryKind.pipeline, :final pipeline?) =>
        pipeline.id,
      TransformEntry(kind: TransformEntryKind.recent, :final recent?) =>
        recent.pipelineId,
      _ => null,
    };
  }

  Future<void> _remember(List<TransformStep> chain, String? pipelineId) async {
    final recent = pipelineId != null
        ? TransformRecent(pipelineId: pipelineId)
        : TransformRecent(steps: chain);
    _recents = [
      recent,
      ..._recents.where((entry) => entry.identity != recent.identity),
    ].take(maxRecents).toList(growable: false);
    await _recentsStore.write(_recents);
  }

  List<TransformStep> _candidateSteps() {
    final candidate = _candidate;
    if (candidate == null) return const [];
    return switch (candidate.kind) {
      TransformEntryKind.transform => [
        candidate.definition!.step(_candidateStepParams()),
      ],
      TransformEntryKind.pipeline => candidate.pipeline!.steps,
      TransformEntryKind.recent => _recentSteps(candidate.recent!),
    };
  }

  Map<String, Object?> _candidateStepParams() =>
      _candidateParams ??
      _preferences.paramDefaults[_candidate!.definition!.id] ??
      const {};

  List<TransformStep> _recentSteps(TransformRecent recent) {
    if (recent.pipelineId case final id?) {
      return _pipelines.where((p) => p.id == id).firstOrNull?.steps ?? const [];
    }
    return recent.steps;
  }

  TransformEntry? _favoriteEntry(TransformFavorite favorite) =>
      switch (favorite.kind) {
        TransformFavoriteKind.transform => switch (_byId[favorite.id]) {
          final definition? => TransformEntry.transform(definition),
          null => null,
        },
        TransformFavoriteKind.pipeline => switch (_pipelines
            .where((p) => p.id == favorite.id)
            .firstOrNull) {
          final pipeline? => _pipelineEntry(pipeline),
          null => null,
        },
      };

  TransformEntry _pipelineEntry(TransformPipeline pipeline) =>
      TransformEntry.pipeline(
        pipeline,
        unavailable: pipeline.steps.any(
          (step) =>
              _byId[step.transformId] == null ||
              step.version > _byId[step.transformId]!.version,
        ),
      );

  static String _chainKey(List<TransformStep> chain) =>
      jsonEncode([for (final step in chain) step.toJson()]);

  void _schedulePreview() {
    if (_disposed || _status != TransformPaletteStatus.ready) return;
    _debounceTimer?.cancel();
    _previewError = null;
    if (chain.isEmpty) {
      cancelPreview();
      _preview = null;
      _previewChain = null;
      notifyListeners();
      return;
    }
    _previewPending = true;
    _debounceTimer = Timer(debounce, () => unawaited(_runPreview()));
  }

  Future<void> _runPreview() async {
    final chain = this.chain;
    final key = _chainKey(chain);
    final sequence = ++_sequence;
    final request = 'transform-palette-${identityHashCode(this)}-$sequence';
    final previous = _inFlight;
    _inFlight = request;
    _clearProgress();
    _spinnerTimer = Timer(spinnerDelay, () {
      _showSpinner = true;
      notifyListeners();
    });
    _cancelTimer = Timer(cancelDelay, () {
      _showCancel = true;
      notifyListeners();
    });
    if (previous != null) unawaited(_cancelRequest(previous));
    try {
      final preview = await _repository.preview(
        request,
        source,
        chain,
        withInput: true,
      );
      if (_disposed || sequence != _sequence) return;
      _preview = preview;
      _previewChain = key;
      _previewError = null;
    } on TransformException catch (error) {
      if (_disposed || sequence != _sequence) return;
      _sourceFailed(error);
    } finally {
      if (!_disposed && sequence == _sequence) {
        _inFlight = null;
        _previewPending = false;
        _clearProgress();
        notifyListeners();
      }
    }
  }

  void _sourceFailed(TransformException error) {
    switch (error.code) {
      case TransformException.unsupportedContent:
        _status = TransformPaletteStatus.notText;
        _cancelTimers();
      case TransformException.notFound
          when source is! TransformTextSource && _preview == null:
        _status = TransformPaletteStatus.empty;
        _cancelTimers();
      case TransformException.contentTooLarge:
        _previewError = AppStrings.current.transformsErrorTooLarge;
      default:
        _previewError = error.message;
    }
    notifyListeners();
  }

  Future<void> _cancelRequest(String request) async {
    try {
      await _repository.cancel(request);
    } on TransformException {
      // A finished preview has nothing left to cancel.
    }
  }

  void _clearProgress() {
    _spinnerTimer?.cancel();
    _cancelTimer?.cancel();
    _spinnerTimer = null;
    _cancelTimer = null;
    _showSpinner = false;
    _showCancel = false;
  }

  void _cancelTimers() {
    _debounceTimer?.cancel();
    _debounceTimer = null;
    _previewPending = false;
    _clearProgress();
  }

  @override
  void notifyListeners() {
    if (!_disposed) super.notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _cancelTimers();
    final request = _inFlight;
    _inFlight = null;
    if (request != null) unawaited(_cancelRequest(request));
    super.dispose();
  }
}
