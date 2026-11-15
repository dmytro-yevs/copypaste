import 'dart:async';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../../l10n/l10n.dart';
import '../../platform/desktop/quick_paste_host.dart';
import '../../platform/security/device_authenticator.dart';
import '../history/controller/history_controller.dart';
import '../history/controller/history_window_controller.dart';
import '../paste_queue/controller/paste_queue_controller.dart';
import '../paste_queue/models/paste_queue_models.dart';
import '../../shared/error_text.dart';
import '../../platform/security/owner_access.dart';
import '../modules/models/module_models.dart';
import '../modules/models/module_output.dart';
import '../modules/models/module_surfaces.dart';
import '../modules/models/module_view.dart';
import '../modules/repository/module_text_delivery.dart';
import '../modules/repository/modules_repository.dart';
import '../transfers/file_operations_controller.dart';
import '../history/controller/history_ocr_controller.dart';
import '../history/models/history_models.dart';
import '../history/repository/history_file_downloader.dart';
import '../history/repository/history_file_transfer.dart';
import '../history/repository/history_repository.dart';
import '../settings/repository/quick_paste_preferences_store.dart';
import '../transforms/models/transform_models.dart';
import '../transforms/repository/transform_recents_store.dart';
import '../transforms/repository/transform_repository.dart';
import 'repository/quick_paste_delivery_repository.dart';

enum QuickPasteTabKind { history, module, queue }

/// History first, then one tab per module with a Quick Paste surface, then
/// the paste queue.
@immutable
class QuickPasteTab {
  const QuickPasteTab._(this.kind, [this.moduleId]);

  const QuickPasteTab.module(String moduleId)
    : this._(QuickPasteTabKind.module, moduleId);

  static const history = QuickPasteTab._(QuickPasteTabKind.history);
  static const queue = QuickPasteTab._(QuickPasteTabKind.queue);

  final QuickPasteTabKind kind;
  final String? moduleId;

  @override
  bool operator ==(Object other) =>
      other is QuickPasteTab &&
      other.kind == kind &&
      other.moduleId == moduleId;

  @override
  int get hashCode => Object.hash(kind, moduleId);
}

/// One result of a module's Quick Paste surface.
class QuickPasteModuleResult {
  const QuickPasteModuleResult(this.moduleId, this.item);

  final String moduleId;
  final ModuleResultItem item;

  bool sameAs(QuickPasteModuleResult? other) =>
      other != null && other.moduleId == moduleId && other.item.id == item.id;
}

/// Where a module delivery goes: pasted or copied as the activation asks,
/// copied only, or added to the paste queue.
enum QuickPasteRoute { activate, copy, queue }

/// A view a module returned for a result, such as a template form, shown in
/// place of the list until it delivers.
class QuickPasteModuleForm {
  const QuickPasteModuleForm({
    required this.moduleId,
    required this.title,
    required this.view,
    required this.route,
    required this.invertAutoPaste,
    required this.secret,
  });

  final String moduleId;
  final String title;
  final ModuleView view;
  final QuickPasteRoute route;
  final bool invertAutoPaste;

  /// The result it belongs to is secret.
  final bool secret;

  QuickPasteModuleForm withView(ModuleView view) => QuickPasteModuleForm(
    moduleId: moduleId,
    title: title,
    view: view,
    route: route,
    invertAutoPaste: invertAutoPaste,
    secret: secret,
  );
}

enum QuickPasteNoticeKind {
  /// The output is on the clipboard but could not be pasted.
  copiedOnly,

  /// The default paste transform failed; the original was used.
  defaultSkipped,
  addedToQueue,
  failed,
}

class QuickPasteNotice {
  const QuickPasteNotice(this.kind, {this.count = 0, this.message});

  final QuickPasteNoticeKind kind;
  final int count;
  final String? message;
}

/// Builds the paste queue on first use; [hideWindow] hides the popup before
/// a queue paste reaches the target application.
typedef QuickPasteQueueFactory =
    PasteQueueController Function(Future<void> Function() hideWindow);

class QuickPasteController extends ChangeNotifier {
  static const _inspectorHoverDelay = Duration(milliseconds: 1500);

  /// Delay before a restore puts the user's clipboard back after a paste.
  static const restoreDelay = Duration(milliseconds: 500);

  QuickPasteController({
    required HistoryRepository repository,
    Future<void> Function()? disposeRepository,
    required QuickPastePreferencesStore preferencesStore,
    required QuickPasteContextHost host,
    HistoryFileDownloader? fileDownloader,
    HistoryFileTransfer? fileTransfer,
    FileOperationsController? fileOperations,
    HistoryOcrController? ocr,
    Future<void> Function()? initializeInspector,
    ModulesRepository? modules,
    ModuleTextDelivery? moduleDelivery,
    TransformRepository? transforms,
    this.transformRecents,
    QuickPasteDeliveryRepository? delivery,
    DeviceAuthenticator? authenticator,
    QuickPasteQueueFactory? createQueue,
    DateTime Function()? clock,
  }) : _disposeRepository = disposeRepository,
       _initializeInspector = initializeInspector,
       _preferencesStore = preferencesStore,
       _host = host,
       _modules = modules,
       _moduleDelivery = moduleDelivery,
       _transforms = transforms,
       _delivery = delivery,
       _createQueue = createQueue,
       _secretAccess = authenticator == null
           ? null
           : OwnerAccess(authenticator: authenticator, clock: clock),
       history = HistoryController(
         repository,
         pageSize: 50,
         includeFacets: false,
         separatePinnedWindow: true,
         searchDebounce: Duration.zero,
         fileDownloader: fileDownloader,
         fileTransfer: fileTransfer,
         fileOperations: fileOperations,
         ocr: ocr,
       ),
       _preferences = QuickPastePreferences.defaults() {
    history.addListener(_historyChanged);
    _host.setOpenedHandler(opened);
    _host.setShutdownHandler(shutdown);
  }

  final Future<void> Function()? _disposeRepository;
  final Future<void> Function()? _initializeInspector;
  Future<void>? _inspectorInitialization;
  final QuickPastePreferencesStore _preferencesStore;
  final QuickPasteContextHost _host;
  final HistoryController history;

  QuickPastePreferences _preferences;
  HistoryClip? _focusedClip;
  String? _inspectedClipId;
  bool _accessibilityGranted = false;
  bool _initialized = false;
  bool _initialHistoryReady = false;
  bool _activating = false;
  int _activationGeneration = 0;
  bool _historyDisposed = false;
  int _presentationGeneration = 0;
  int? _presentationId;
  int? _renderablePresentationId;
  bool _disposed = false;
  bool _inspectorOpen = false;
  Map<String, String> _pinnedShortcuts = {};
  final Map<String, HistoryClip> _shortcutClips = {};
  HistoryQuery? _shortcutClipQuery;
  Future<void> _shortcutSync = Future<void>.value();
  List<(String, bool)>? _scheduledPinState;
  final Set<String> _pendingDeletedPinIds = {};
  List<HistoryClip>? _derivedSource;
  TargetPlatform? _derivedPlatform;
  List<HistoryClip> _derivedItems = const [];
  Map<LogicalKeyboardKey, HistoryClip> _derivedShortcuts = const {};
  Map<String, LogicalKeyboardKey> _derivedClipKeys = const {};
  Future<void>? _repositoryDisposal;
  Future<void>? _activeAction;
  Future<void>? _pageLoad;
  bool _paginationFailed = false;
  int _focusGeneration = 0;
  Timer? _inspectorHoverTimer;
  String? _hoveredClipId;
  bool _autoOpenSuppressed = false;
  bool? _requestedInspectorVisible;
  Future<void> _inspectorChanges = Future<void>.value();

  final ModulesRepository? _modules;
  final ModuleTextDelivery? _moduleDelivery;
  final TransformRepository? _transforms;
  final TransformRecentsStore? transformRecents;
  final QuickPasteDeliveryRepository? _delivery;
  final QuickPasteQueueFactory? _createQueue;
  final OwnerAccess? _secretAccess;
  PasteQueueController? _queue;
  QuickPasteTab _tab = QuickPasteTab.history;
  String _query = '';
  List<(String, ModuleQuickPasteSurface)> _surfaces = const [];
  Map<String, List<ModuleResultItem>> _moduleItems = const {};
  bool _resultsLoading = false;
  bool _resultsFailed = false;
  int _resultsRequest = 0;
  QuickPasteModuleResult? _focusedResult;
  QuickPasteModuleForm? _moduleForm;
  bool _maskedMode = false;
  TransformPipeline? _defaultPipeline;
  Timer? _unlockTimer;
  QuickPasteNotice? _notice;
  int _noticeRevision = 0;

  /// Asks the user to confirm an unlock where the device cannot verify its
  /// owner; set by the view, which can show the question.
  Future<bool> Function()? confirmUnlock;

  QuickPasteTab get tab => _tab;

  List<QuickPasteTab> get tabs => [
    QuickPasteTab.history,
    for (final (id, _) in _surfaces) QuickPasteTab.module(id),
    if (_createQueue != null) QuickPasteTab.queue,
  ];

  /// The title of a module tab; null for the built-in tabs.
  String? moduleTabTitle(QuickPasteTab tab) => _surface(tab.moduleId)?.title;

  ModuleQuickPasteSurface? _surface(String? moduleId) =>
      _surfaces.where((entry) => entry.$1 == moduleId).firstOrNull?.$2;

  /// A module tab's results, or in the History tab while searching each
  /// module's first `mixed_limit` results above history, in module order.
  List<QuickPasteModuleResult> get moduleResults {
    if (_tab.kind == QuickPasteTabKind.module) {
      final id = _tab.moduleId!;
      return [
        for (final item in _moduleItems[id] ?? const <ModuleResultItem>[])
          QuickPasteModuleResult(id, item),
      ];
    }
    if (_tab != QuickPasteTab.history || _query.trim().isEmpty) return const [];
    return [
      for (final (id, surface) in _surfaces)
        for (final item
            in (_moduleItems[id] ?? const <ModuleResultItem>[]).take(
              surface.mixedLimit,
            ))
          QuickPasteModuleResult(id, item),
    ];
  }

  bool get moduleResultsLoading => _resultsLoading;
  bool get moduleResultsFailed => _resultsFailed;
  String get query => _query;
  QuickPasteModuleResult? get focusedResult => _focusedResult;
  QuickPasteModuleForm? get moduleForm => _moduleForm;
  PasteQueueController? get queue => _queue;
  TransformRepository? get transforms => _transforms;

  /// The paste target is a password field: previews are masked, transforms
  /// are off and pastes update no usage or recency.
  bool get maskedMode => _maskedMode;

  bool get secretsLocked {
    final access = _secretAccess;
    return access != null && !access.unlocked;
  }

  bool get canTransform => _transforms != null && !_maskedMode;

  /// The pipeline Enter pastes text through; Mod+Shift+Enter bypasses it.
  TransformPipeline? get defaultPipeline =>
      _maskedMode ? null : _defaultPipeline;

  QuickPasteNotice? get notice => _notice;
  int get noticeRevision => _noticeRevision;

  bool get autoPaste => _preferences.autoPaste;
  bool get accessibilityGranted => _accessibilityGranted;
  bool get activating => _activating;
  double? transferProgressFor(String id) =>
      history.transfers.downloadProgressFor(id);
  bool transferFailedFor(String id) => history.transfers.downloadFailedFor(id);
  int get presentationGeneration => _presentationGeneration;
  HistoryClip? get focusedClip => _focusedClip;
  bool get inspectorOpen => _inspectorOpen;
  bool get paginationFailed => _paginationFailed;
  int? get renderablePresentationId =>
      _host is QuickPasteFrameHost &&
          _presentationId == _renderablePresentationId &&
          history.window.state != HistoryLoadState.initial &&
          history.window.state != HistoryLoadState.loading
      ? _presentationId
      : null;

  Future<bool> presentFrame(int presentationId) async {
    if (!_isCurrent(presentationId) ||
        renderablePresentationId != presentationId) {
      return false;
    }
    final host = _host;
    if (host is! QuickPasteFrameHost) return true;
    return (host as QuickPasteFrameHost).presentFrame(
      presentationId: presentationId,
    );
  }

  List<HistoryClip> get items {
    _ensureDerived();
    return _derivedItems;
  }

  Map<String, LogicalKeyboardKey> get _availablePinKeys =>
      defaultTargetPlatform == TargetPlatform.macOS ? _pinKeys : _otherPinKeys;

  Map<LogicalKeyboardKey, HistoryClip> get shortcuts {
    _ensureDerived();
    return _derivedShortcuts;
  }

  LogicalKeyboardKey? shortcutFor(HistoryClip clip) {
    _ensureDerived();
    return _derivedClipKeys[clip.id];
  }

  void _ensureDerived() {
    final platform = defaultTargetPlatform;
    if (_derivedSource != null && platform == _derivedPlatform) return;
    final source = history.window.items;
    if (!identical(_shortcutClipQuery, history.window.query) ||
        history.window.state == HistoryLoadState.initial) {
      _shortcutClipQuery = history.window.query;
      _shortcutClips.clear();
    }
    _shortcutClips.removeWhere(
      (id, _) =>
          !_pinnedShortcuts.containsKey(id) ||
          !history.window.isVisitedPinned(id),
    );
    for (final clip in source) {
      if (clip.pinned && _pinnedShortcuts.containsKey(clip.id)) {
        // Activation resolves the full clip by ID; keep no preview payload.
        _shortcutClips[clip.id] = HistoryClip(
          id: clip.id,
          contentType: '',
          preview: '',
          createdAt: clip.createdAt,
          pinned: true,
        );
      }
    }
    for (final id in history.window.visitedPinnedIds) {
      if (!_pinnedShortcuts.containsKey(id) || _shortcutClips.containsKey(id)) {
        continue;
      }
      // Pinned shortcut activation resolves the complete summary by ID.
      // The compact key binding never needs an evicted preview payload.
      _shortcutClips[id] = HistoryClip(
        id: id,
        contentType: '',
        preview: '',
        createdAt: DateTime.fromMillisecondsSinceEpoch(0),
        pinned: true,
      );
    }
    _derivedSource = source;
    _derivedPlatform = platform;
    final recent = <HistoryClip>[];
    final pinned = <HistoryClip>[];
    for (final clip in source) {
      (clip.pinned ? pinned : recent).add(clip);
    }
    _derivedItems = List.unmodifiable([...recent, ...pinned]);
    final keys = <LogicalKeyboardKey, HistoryClip>{
      for (final (index, clip) in recent.take(9).indexed)
        _numberKeys[index]: clip,
      for (final clip in _shortcutClips.values)
        ?_availablePinKeys[_pinnedShortcuts[clip.id]]: clip,
    };
    _derivedShortcuts = Map.unmodifiable(keys);
    _derivedClipKeys = Map.unmodifiable({
      for (final entry in keys.entries) entry.value.id: entry.key,
    });
  }

  Future<void> initialize() async {
    if (_disposed || _initialized) return;
    _initialized = true;
    await Future.wait([
      () async {
        _preferences = await _preferencesStore.read();
      }(),
      () async {
        _pinnedShortcuts = await _preferencesStore.readPinnedShortcuts();
        _derivedSource = null;
      }(),
    ]);
    if (_disposed) return;
    await Future.wait([
      history.initialize(),
      _refreshAccessibility(),
      _loadModules(),
    ]);
    if (_disposed) return;
    await _reconcilePinnedShortcuts();
    if (_disposed) return;
    _initialHistoryReady = true;
    await _host.signalReady();
    if (!_disposed) notifyListeners();
  }

  /// [targetApp] identifies the application that receives the paste, in the
  /// form per-app transform defaults are keyed by, when the host knows it.
  Future<void> opened(
    int presentationId, {
    bool inspectorVisible = false,
    String? targetApp,
  }) async {
    if (_disposed || presentationId <= 0) return;
    final query = history.window.query;
    // Native opens the first presentation after this watched page is ready.
    // Re-fetching it clears already rendered rows and duplicates startup I/O.
    final reuseInitialHistory =
        _initialHistoryReady &&
        _presentationGeneration == 0 &&
        (history.window.state == HistoryLoadState.ready ||
            history.window.state == HistoryLoadState.empty) &&
        !query.hasSearch &&
        query.kind == null &&
        !query.pinnedOnly &&
        query.origin == null &&
        query.sourceApp == null &&
        !query.hasDateFilter &&
        !query.hasTagFilter &&
        query.sort == HistorySort.newest;
    _cancelInspectorHover();
    _endPresentationState();
    _autoOpenSuppressed = false;
    _requestedInspectorVisible = null;
    _presentationId = presentationId;
    _renderablePresentationId = null;
    _inspectorOpen = inspectorVisible;
    if (_inspectorOpen) _prepareInspector();
    _focusedClip = null;
    _focusGeneration += 1;
    _pageLoad = null;
    _paginationFailed = false;
    _presentationGeneration += 1;
    final concealed = history.spoilers.setRevealed(null, false);
    notifyListeners();
    await concealed;
    if (!_isCurrent(presentationId)) return;
    final preferences = await _preferencesStore.read();
    if (!_isCurrent(presentationId)) return;
    _preferences = preferences;
    // The target is sampled once per presentation, before anything renders.
    await Future.wait([_detectMaskedMode(), _loadDefaultPipeline(targetApp)]);
    if (!_isCurrent(presentationId)) return;
    if (_maskedMode && _inspectorOpen) {
      _inspectorOpen = false;
      unawaited(_hideInspectorFor(presentationId));
    }
    unawaited(
      _loadModules().then((_) async {
        if (_isCurrent(presentationId) &&
            _tab.kind == QuickPasteTabKind.module) {
          await _refreshModuleResults();
        }
      }),
    );
    if (!reuseInitialHistory) {
      await history.window.updateQuery(const HistoryQuery());
      if (!_isCurrent(presentationId)) return;
      await _reconcilePinnedShortcuts();
      if (!_isCurrent(presentationId)) return;
    }
    await _refreshAccessibility();
    if (_inspectorOpen && _isCurrent(presentationId) && items.isNotEmpty) {
      await history.selection.select(items.first.id);
    }
    if (_isCurrent(presentationId)) {
      _renderablePresentationId = presentationId;
      notifyListeners();
    }
  }

  void _prepareInspector() {
    final initialize = _initializeInspector;
    if (_disposed || initialize == null || _inspectorInitialization != null) {
      return;
    }
    late final Future<void> pending;
    pending = initialize().catchError((Object _) {}).whenComplete(() {
      if (identical(_inspectorInitialization, pending)) {
        _inspectorInitialization = null;
      }
    });
    _inspectorInitialization = pending;
  }

  bool _isCurrent(int presentationId) =>
      !_disposed && _presentationId == presentationId;

  Future<void> search(String query) {
    _cancelInspectorHover();
    _autoOpenSuppressed = false;
    _focusedClip = null;
    _focusedResult = null;
    _moduleForm = null;
    _focusGeneration += 1;
    _pageLoad = null;
    _paginationFailed = false;
    _query = query;
    unawaited(_refreshModuleResults());
    final sort = query.trim().isEmpty
        ? HistorySort.newest
        : HistorySort.relevance;
    return history.window.updateQuery(HistoryQuery(search: query, sort: sort));
  }

  Future<void> loadMore() {
    if (_disposed) return Future<void>.value();
    final pending = _pageLoad;
    if (pending != null) return pending;
    final generation = _focusGeneration;
    final marker = history.window.paginationMarker;
    _paginationFailed = false;
    late final Future<void> operation;
    operation = history.window.loadMore().whenComplete(() {
      if (identical(_pageLoad, operation)) _pageLoad = null;
      if (!_disposed && generation == _focusGeneration) {
        _paginationFailed =
            history.window.paginationMarker == marker &&
            history.window.canLoadMore;
        notifyListeners();
      }
    });
    _pageLoad = operation;
    return operation;
  }

  /// Moves through the module results, then the history rows below them.
  Future<void> moveFocus(int offset) async {
    if (_tab == QuickPasteTab.queue || _moduleForm != null || offset == 0) {
      return;
    }
    final rows = moduleResults;
    final result = _focusedResult;
    final moduleTab = _tab.kind == QuickPasteTabKind.module;
    if (moduleTab || result != null) {
      final index = result == null
          ? -1
          : rows.indexWhere((row) => row.sameAs(result));
      final next = index < 0 ? 0 : index + offset;
      if (next >= 0 && next < rows.length) {
        focusResult(rows[next]);
      } else if (moduleTab && rows.isNotEmpty) {
        focusResult(rows[next.clamp(0, rows.length - 1)]);
      } else if (offset > 0 && _tab == QuickPasteTab.history) {
        final first = items.firstOrNull;
        if (first != null) focus(first);
      }
      return;
    }
    final clip = _focusedClip;
    if (rows.isNotEmpty &&
        ((offset > 0 && clip == null) ||
            (offset < 0 &&
                (clip == null || clip.id == items.firstOrNull?.id)))) {
      focusResult(offset > 0 ? rows.first : rows.last);
      return;
    }
    await _moveHistoryFocus(offset);
  }

  void focusResult(QuickPasteModuleResult result) {
    if (result.sameAs(_focusedResult)) return;
    _cancelInspectorHover();
    _focusedResult = result;
    _focusedClip = null;
    _focusGeneration += 1;
    notifyListeners();
  }

  /// Traverses data rather than mounted rows, including page boundaries.
  Future<void> _moveHistoryFocus(int offset) async {
    final generation = _focusGeneration;
    var current = _focusedClip;
    final focusedId = current?.id;
    for (var step = 0; step < offset.abs(); step++) {
      if (_disposed || generation != _focusGeneration) return;
      var visible = items;
      if (visible.isEmpty) return;
      if (current == null) {
        current = visible.first;
        continue;
      }
      var group = current.pinned
          ? history.window.visitedPinnedIds
          : history.window.visitedRecentIds;
      var index = group.indexOf(current.id);
      if (offset > 0 &&
          !current.pinned &&
          index == group.length - 1 &&
          history.window.canLoadMore) {
        final marker = history.window.paginationMarker;
        await loadMore();
        if (_disposed || generation != _focusGeneration) return;
        if (history.window.paginationMarker == marker &&
            history.window.canLoadMore) {
          break;
        }
        group = history.window.visitedRecentIds;
        index = group.indexOf(current.id);
      } else if (offset < 0 &&
          current.pinned &&
          history.window.canLoadPinnedPrevious &&
          visible.where((clip) => clip.pinned).firstOrNull?.id == current.id) {
        await history.window.loadPinnedPrevious();
      } else if (offset > 0 &&
          current.pinned &&
          history.window.canLoadPinnedMore &&
          visible.where((clip) => clip.pinned).lastOrNull?.id == current.id) {
        await history.window.loadPinnedMore();
      } else if (offset < 0 &&
          !current.pinned &&
          history.window.canLoadPrevious &&
          visible.where((clip) => !clip.pinned).firstOrNull?.id == current.id) {
        await history.window.loadPrevious();
      }
      if (_disposed || generation != _focusGeneration) return;
      visible = items;
      final nextIndex = index + offset.sign;
      if (index >= 0 && nextIndex >= 0 && nextIndex < group.length) {
        final target = group[nextIndex];
        final next =
            visible.where((clip) => clip.id == target).firstOrNull ??
            await history.window.materializeId(target);
        if (next != null) {
          current = next;
          continue;
        }
      }
      final visibleIndex = visible.indexWhere((clip) => clip.id == current?.id);
      final next = visibleIndex + offset.sign;
      if (visibleIndex < 0 || next < 0 || next >= visible.length) break;
      current = visible[next];
    }
    if (!_disposed &&
        generation == _focusGeneration &&
        _focusedClip?.id == focusedId &&
        current != null) {
      focus(current);
    }
  }

  void focus(HistoryClip clip) {
    if (_focusedClip?.id == clip.id && _focusedResult == null) return;
    _cancelInspectorHover();
    _autoOpenSuppressed = false;
    _focusedResult = null;
    _focusedClip = clip;
    if (_inspectorOpen) unawaited(history.selection.select(clip.id));
    notifyListeners();
  }

  void hoverClip(HistoryClip clip, {required bool hovered}) {
    if (_disposed) return;
    if (!hovered) {
      if (_hoveredClipId == clip.id) _cancelInspectorHover();
      return;
    }
    // Hover reports every pointer move; only entering a row restarts it.
    if (_hoveredClipId == clip.id && _focusedClip?.id == clip.id) return;
    focus(clip);
    _hoveredClipId = clip.id;
    final id = _presentationId;
    if (id == null ||
        _activating ||
        _maskedMode ||
        (_requestedInspectorVisible ?? _inspectorOpen) ||
        _autoOpenSuppressed) {
      return;
    }
    _inspectorHoverTimer?.cancel();
    _inspectorHoverTimer = Timer(_inspectorHoverDelay, () {
      _inspectorHoverTimer = null;
      if (_isCurrent(id) &&
          !_activating &&
          !_autoOpenSuppressed &&
          _hoveredClipId == clip.id &&
          _focusedClip?.id == clip.id) {
        unawaited(_setInspectorVisible(true, remember: false));
      }
    });
  }

  void _cancelInspectorHover() {
    _inspectorHoverTimer?.cancel();
    _inspectorHoverTimer = null;
    _hoveredClipId = null;
  }

  Future<void> toggleInspector() {
    final id = _presentationId;
    if (id == null || _disposed || _activating || _maskedMode) {
      return Future<void>.value();
    }
    _cancelInspectorHover();
    final visible = !(_requestedInspectorVisible ?? _inspectorOpen);
    _autoOpenSuppressed = !visible;
    return _setInspectorVisible(visible);
  }

  Future<void> _setInspectorVisible(bool visible, {bool remember = true}) {
    final id = _presentationId;
    if (id == null || _disposed || _activating) return Future<void>.value();
    _requestedInspectorVisible = visible;
    late final Future<void> operation;
    operation = _inspectorChanges
        .then((_) async {
          if (!_isCurrent(id) || _activating || _inspectorOpen == visible) {
            return;
          }
          try {
            await _host.setInspectorVisible(
              presentationId: id,
              visible: visible,
              remember: remember,
            );
          } on PlatformException {
            return;
          } on MissingPluginException {
            return;
          }
          if (!_isCurrent(id)) return;
          _inspectorOpen = visible;
          if (visible) {
            _prepareInspector();
            final clip = _focusedClip ?? items.firstOrNull;
            if (clip != null) unawaited(history.selection.select(clip.id));
          }
          if (_isCurrent(id)) notifyListeners();
        })
        .whenComplete(() {
          if (identical(_inspectorChanges, operation)) {
            _requestedInspectorVisible = null;
          }
        });
    _inspectorChanges = operation;
    return operation;
  }

  /// [bypassDefault] pastes the original instead of the default pipeline's
  /// output.
  Future<void> activate(
    HistoryClip clip, {
    bool plainText = false,
    bool invertAutoPaste = false,
    bool forcePaste = false,
    bool bypassDefault = false,
  }) async {
    final presentationId = _presentationId;
    if (_disposed ||
        _activating ||
        presentationId == null ||
        history.transfers.isDownloadPending(clip.id)) {
      return;
    }
    final generation = ++_activationGeneration;
    final focusGeneration = _focusGeneration;
    final focusedId = _focusedClip?.id;
    bool current() =>
        _isCurrent(presentationId) && generation == _activationGeneration;
    _cancelInspectorHover();
    _activating = true;
    final actionDone = Completer<void>();
    _activeAction = actionDone.future;
    notifyListeners();
    try {
      if (clip.secret &&
          !await _ensureSecretAccess(DeviceAuthPurpose.unlockSecrets)) {
        return;
      }
      if (!current()) return;
      await history.selection.select(clip.id);
      if (!current()) return;
      final selected = history.selection.selectedClip;
      if (selected?.contentKind == HistoryClipKind.file &&
          selected?.file != null &&
          !selected!.file!.sourceAvailable) {
        // The runtime owns file I/O; other clipboard actions remain available.
        _activating = false;
        notifyListeners();
        final path = await history.transfers.prepareFileForCopy(selected);
        if (path == null ||
            !current() ||
            focusGeneration != _focusGeneration ||
            focusedId != _focusedClip?.id) {
          return;
        }
        _activating = true;
        await history.selection.select(clip.id);
        if (!current()) return;
      }
      final shouldPaste = forcePaste
          ? true
          : invertAutoPaste
          ? !_preferences.autoPaste
          : _preferences.autoPaste;
      if (await _pasteTransient(
        presentationId,
        selected ?? clip,
        current,
        shouldPaste: shouldPaste,
        plainText: plainText,
        bypassDefault: bypassDefault,
      )) {
        return;
      }
      if (!current()) return;
      final copied = await history.transfers.copySelected(plainText: plainText);
      if (!copied || !current()) return;
      if (shouldPaste) {
        try {
          await _prepareAccessibilityForPaste(presentationId);
        } catch (_) {
          // A permission or preference failure still leaves a completed Copy.
        }
        if (!current()) return;
      }
      if (shouldPaste && _accessibilityGranted) {
        var pasted = false;
        try {
          pasted = await _host.paste(presentationId: presentationId);
        } on PlatformException {
          // Copy already completed; an unavailable paste channel is copy-only.
        } on MissingPluginException {
          // A disappearing native context cannot undo the completed Copy.
        }
        if (pasted) _endPresentationState();
        if (!current() || pasted) return;
      }
      await _closePresentation(presentationId);
    } finally {
      try {
        if (generation == _activationGeneration) {
          _activating = false;
        }
        if (_disposed) _disposeHistory();
        if (!_disposed) notifyListeners();
      } finally {
        actionDone.complete();
        if (identical(_activeAction, actionDone.future)) _activeAction = null;
      }
    }
  }

  Future<void> activateFocused({
    bool plainText = false,
    bool invertAutoPaste = false,
    bool forcePaste = false,
    bool bypassDefault = false,
  }) async {
    if (_tab == QuickPasteTab.queue || _moduleForm != null) return;
    final result = _focusedOrFirstResult;
    if (result != null) {
      await activateResult(result, invertAutoPaste: invertAutoPaste);
      return;
    }
    if (_tab != QuickPasteTab.history) return;
    final clip = _focusedClip ?? items.firstOrNull;
    if (clip == null) return;
    await activate(
      clip,
      plainText: plainText,
      invertAutoPaste: invertAutoPaste,
      forcePaste: forcePaste,
      bypassDefault: bypassDefault,
    );
  }

  /// Pastes a text clip without promoting it: through the default pipeline,
  /// or as itself for a secret (restored afterwards) or a masked target.
  /// Answers whether it handled the activation.
  Future<bool> _pasteTransient(
    int presentationId,
    HistoryClip clip,
    bool Function() current, {
    required bool shouldPaste,
    required bool plainText,
    required bool bypassDefault,
  }) async {
    final transforms = _transforms;
    if (transforms == null ||
        !shouldPaste ||
        plainText ||
        !clip.contentKind.isTextual) {
      return false;
    }
    final pipeline = bypassDefault ? null : defaultPipeline;
    if (pipeline == null && !clip.secret && !_maskedMode) return false;
    try {
      await _prepareAccessibilityForPaste(presentationId);
    } catch (_) {
      // The ordinary activation decides how to finish without a permission.
    }
    if (!current() || !_accessibilityGranted) return false;
    String? token;
    try {
      final result = await transforms.run(
        TransformSource.clip(clip.id),
        pipelineId: pipeline?.id,
        action: TransformRunAction.paste,
        bypassDefault: pipeline == null,
      );
      token = result.restoreToken;
    } on TransformException {
      if (pipeline != null) {
        _announce(const QuickPasteNotice(QuickPasteNoticeKind.defaultSkipped));
      }
      return false;
    }
    if (!current()) return true;
    var pasted = false;
    try {
      pasted = await _host.paste(presentationId: presentationId);
    } on PlatformException {
      // The content stays on the clipboard for a manual paste.
    } on MissingPluginException {
      // A disappearing native context leaves the content on the clipboard.
    }
    if (pasted) {
      // Only a secret or produced output puts the user's clipboard back.
      if (token != null && (clip.secret || pipeline != null)) {
        unawaited(_restoreClipboard(token));
      }
      _endPresentationState();
      return true;
    }
    if (current()) await _closePresentation(presentationId);
    return true;
  }

  Future<void> selectTab(QuickPasteTab tab) async {
    if (_disposed || _tab == tab || !tabs.contains(tab)) return;
    _cancelInspectorHover();
    _moduleForm = null;
    _tab = tab;
    _focusedResult = null;
    if (tab == QuickPasteTab.queue) _ensureQueue();
    notifyListeners();
    if (tab != QuickPasteTab.queue) await _refreshModuleResults();
  }

  /// Ctrl+Tab moves to the next tab and Ctrl+Shift+Tab to the previous one.
  Future<void> cycleTab(int delta) {
    final available = tabs;
    final index = available.indexOf(_tab);
    return selectTab(available[(index + delta) % available.length]);
  }

  /// Reads the modules with a Quick Paste surface; a failure leaves none.
  Future<void> _loadModules() async {
    final modules = _modules;
    if (modules == null || _disposed) return;
    List<(String, ModuleQuickPasteSurface)> surfaces;
    try {
      surfaces = [
        for (final module in await modules.list())
          if (module.active)
            if (module.surfaces.quickPaste case final surface?)
              (module.id, surface),
      ];
    } on Object {
      surfaces = const [];
    }
    if (_disposed) return;
    _surfaces = List.unmodifiable(surfaces);
    if (!tabs.contains(_tab)) {
      _tab = QuickPasteTab.history;
      _focusedResult = null;
      _moduleForm = null;
    }
    notifyListeners();
  }

  Future<void> _refreshModuleResults() async {
    final modules = _modules;
    if (modules == null || _disposed) return;
    final request = ++_resultsRequest;
    final query = _query.trim();
    final targets = [
      for (final (id, surface) in _surfaces)
        if (_tab.kind == QuickPasteTabKind.module
            ? _tab.moduleId == id
            : _tab == QuickPasteTab.history &&
                  query.isNotEmpty &&
                  surface.mixedLimit > 0)
          (id, surface),
    ];
    if (targets.isEmpty) {
      _moduleItems = const {};
      _resultsLoading = false;
      _resultsFailed = false;
      notifyListeners();
      return;
    }
    // Previous rows stay visible until the new ones arrive.
    _resultsLoading = true;
    notifyListeners();
    var failed = false;
    final items = <String, List<ModuleResultItem>>{};
    await Future.wait([
      for (final (id, surface) in targets)
        () async {
          try {
            final output = await modules.invoke(id, surface.searchCommand, {
              'query': query,
            });
            items[id] = output.results ?? const [];
          } on Object {
            failed = true;
          }
        }(),
    ]);
    if (_disposed || request != _resultsRequest) return;
    _resultsLoading = false;
    _resultsFailed = failed;
    _moduleItems = Map.unmodifiable(items);
    // A module preselects at most one result, such as an exact abbreviation.
    final rows = moduleResults;
    final preselected = rows.where((row) => row.item.preselect).firstOrNull;
    if (preselected != null) {
      _focusedResult = preselected;
      _focusedClip = null;
    } else if (_focusedResult case final focused?
        when !rows.any((row) => row.sameAs(focused))) {
      _focusedResult = null;
    }
    notifyListeners();
  }

  QuickPasteModuleResult? get _focusedOrFirstResult =>
      _focusedResult ??
      (_tab.kind == QuickPasteTabKind.module
          ? moduleResults.firstOrNull
          : null);

  /// Runs the module's activate command for [result]: a delivery is pasted,
  /// copied or queued as [route] asks; a view opens in place of the list.
  Future<void> activateResult(
    QuickPasteModuleResult result, {
    QuickPasteRoute route = QuickPasteRoute.activate,
    bool invertAutoPaste = false,
  }) async {
    final surface = _surface(result.moduleId);
    if (surface == null || _moduleForm != null) return;
    await _moduleAction((presentationId) async {
      final output = await _modules!.invoke(
        result.moduleId,
        surface.activateCommand,
        {'id': result.item.id},
      );
      if (!_isCurrent(presentationId)) return;
      await _handleModuleOutput(
        presentationId,
        output,
        QuickPasteModuleForm(
          moduleId: result.moduleId,
          title: result.item.title,
          view: const ModuleView(sections: []),
          route: route,
          invertAutoPaste: invertAutoPaste,
          secret: result.item.secret,
        ),
      );
    });
  }

  /// Submits the open module form; its delivery goes where the form's
  /// activation asked, and a returned view, such as field errors, replaces it.
  Future<void> submitModuleForm(ModuleViewAction action) async {
    final form = _moduleForm;
    if (form == null) return;
    await _moduleAction((presentationId) async {
      final output = await _modules!.invoke(
        form.moduleId,
        action.command,
        action.arguments,
      );
      if (!_isCurrent(presentationId) || !identical(_moduleForm, form)) {
        return;
      }
      await _handleModuleOutput(presentationId, output, form);
    });
  }

  /// The preview text the open form's preview action renders.
  Future<String?> previewModuleForm(ModuleViewAction action) async {
    final form = _moduleForm;
    final modules = _modules;
    if (form == null || modules == null) return null;
    try {
      final output = await modules.invoke(
        form.moduleId,
        action.command,
        action.arguments,
      );
      return identical(_moduleForm, form) ? output.text : null;
    } on Object {
      return null;
    }
  }

  /// Returns from the module form to the results; the query is kept.
  void closeModuleForm() {
    if (_moduleForm == null) return;
    _moduleForm = null;
    notifyListeners();
  }

  Future<void> _handleModuleOutput(
    int presentationId,
    ModuleResult output,
    QuickPasteModuleForm form,
  ) async {
    if (output.delivery case final delivery?) {
      await _deliverModule(presentationId, delivery, form);
    } else if (output.view case final view?) {
      _moduleForm = form.withView(view);
    } else if (output.text.trim().isNotEmpty) {
      _announce(
        QuickPasteNotice(QuickPasteNoticeKind.failed, message: output.text),
      );
    }
  }

  /// Runs one module request as the presentation's action; failures become
  /// a failed notice.
  Future<void> _moduleAction(
    Future<void> Function(int presentationId) action,
  ) async {
    final presentationId = _presentationId;
    if (_modules == null ||
        presentationId == null ||
        _disposed ||
        _activating) {
      return;
    }
    _cancelInspectorHover();
    _activating = true;
    final actionDone = Completer<void>();
    _activeAction = actionDone.future;
    notifyListeners();
    try {
      await action(presentationId);
    } on ModulesException catch (error) {
      _announce(
        QuickPasteNotice(
          QuickPasteNoticeKind.failed,
          message: ErrorText.reason(error),
        ),
      );
    } on QuickPasteDeliveryException catch (error) {
      _announce(
        QuickPasteNotice(
          QuickPasteNoticeKind.failed,
          message: ErrorText.reason(error),
        ),
      );
    } finally {
      try {
        _activating = false;
        if (_disposed) _disposeHistory();
        if (!_disposed) notifyListeners();
      } finally {
        actionDone.complete();
        if (identical(_activeAction, actionDone.future)) _activeAction = null;
      }
    }
  }

  /// Pastes, copies or queues module text. A password target gets it as a
  /// secret that history does not record.
  Future<void> _deliverModule(
    int presentationId,
    ModuleDelivery delivery,
    QuickPasteModuleForm form,
  ) async {
    final text = _moduleDelivery;
    if (text == null) return;
    if ((delivery.requireAuth || (form.secret && delivery.secret)) &&
        !await _ensureSecretAccess(DeviceAuthPurpose.unlockSecrets)) {
      return;
    }
    if (!_isCurrent(presentationId)) return;
    if (form.route == QuickPasteRoute.queue) {
      _moduleForm = null;
      final queue = _ensureQueue();
      if (queue == null) return;
      final added = await queue.add([
        PasteQueueSource.delivered(
          delivery.text,
          secret: delivery.secret || _maskedMode,
        ),
      ]);
      _announce(
        added
            ? QuickPasteNotice(
                QuickPasteNoticeKind.addedToQueue,
                count: queue.pendingCount,
              )
            : const QuickPasteNotice(QuickPasteNoticeKind.failed),
      );
      return;
    }
    final wantPaste =
        form.route == QuickPasteRoute.activate &&
        (form.invertAutoPaste
            ? !_preferences.autoPaste
            : _preferences.autoPaste);
    if (wantPaste) {
      try {
        await _prepareAccessibilityForPaste(presentationId);
      } catch (_) {
        // Without the permission the text is still copied.
      }
      if (!_isCurrent(presentationId)) return;
    }
    final paste = wantPaste && _accessibilityGranted;
    final delivered = await text.deliver(
      form.moduleId,
      _maskedMode
          ? ModuleDelivery(
              text: delivery.text,
              label: delivery.label,
              secret: true,
              requireAuth: delivery.requireAuth,
              cursorFromEnd: delivery.cursorFromEnd,
            )
          : delivery,
      paste: paste,
    );
    if (!_isCurrent(presentationId)) return;
    if (!paste) {
      if (wantPaste) {
        _moduleForm = null;
        _announce(const QuickPasteNotice(QuickPasteNoticeKind.copiedOnly));
        return;
      }
      await _closePresentation(presentationId);
      return;
    }
    var pasted = false;
    try {
      pasted = await _host.paste(presentationId: presentationId);
    } on PlatformException {
      // The text stays on the clipboard for a manual paste.
    } on MissingPluginException {
      // A disappearing native context leaves the text on the clipboard.
    }
    if (!pasted) {
      // The text stays on the clipboard and is not restored.
      if (_isCurrent(presentationId)) {
        _moduleForm = null;
        _announce(const QuickPasteNotice(QuickPasteNoticeKind.copiedOnly));
      }
      return;
    }
    final host = _host;
    final cursor = delivered.cursorFromEnd;
    if (cursor != null &&
        cursor > 0 &&
        cursor <= ForegroundPasteHost.maxCaretMove &&
        host is ForegroundPasteHost) {
      try {
        await (host as ForegroundPasteHost).moveCaretLeft(cursor);
      } on Object {
        // The caret stays at the end of the pasted text.
      }
    }
    if (delivered.token case final token?) {
      unawaited(
        text.restoreClipboard(token, restoreDelay).catchError((Object _) {}),
      );
    }
    _endPresentationState();
  }

  /// Adds the focused clip or module result to the paste queue.
  Future<void> addFocusedToQueue() async {
    if (_createQueue == null ||
        _disposed ||
        _activating ||
        _tab == QuickPasteTab.queue ||
        _moduleForm != null) {
      return;
    }
    final result = _focusedOrFirstResult;
    if (result != null) {
      await activateResult(result, route: QuickPasteRoute.queue);
      return;
    }
    final clip = _tab == QuickPasteTab.history
        ? _focusedClip ?? items.firstOrNull
        : null;
    if (clip == null) return;
    // Secrets join the queue only after an unlock.
    if (clip.secret &&
        !await _ensureSecretAccess(DeviceAuthPurpose.unlockSecrets)) {
      return;
    }
    final queue = _ensureQueue();
    if (queue == null) return;
    final added = await queue.add([PasteQueueSource.clip(clip.id)]);
    _announce(
      added
          ? QuickPasteNotice(
              QuickPasteNoticeKind.addedToQueue,
              count: queue.pendingCount,
            )
          : const QuickPasteNotice(QuickPasteNoticeKind.failed),
    );
  }

  /// Copies the focused module result without pasting it.
  Future<void> copyFocusedResult() async {
    final result = _focusedOrFirstResult;
    if (result == null || _tab == QuickPasteTab.queue) return;
    await activateResult(result, route: QuickPasteRoute.copy);
  }

  PasteQueueController? _ensureQueue() {
    final create = _createQueue;
    if (create == null || _disposed) return null;
    var queue = _queue;
    if (queue == null) {
      queue = _queue = create(close);
      unawaited(queue.start());
    }
    return queue;
  }

  /// Unlocks secret items for this presentation, at most for
  /// [OwnerAccess.window].
  Future<bool> unlockSecrets() =>
      _ensureSecretAccess(DeviceAuthPurpose.unlockSecrets);

  Future<bool> _ensureSecretAccess(DeviceAuthPurpose purpose) async {
    final access = _secretAccess;
    if (access == null || access.unlocked) return true;
    final result = await access.request(
      deviceAuthReason(AppStrings.current, purpose),
    );
    if (_disposed || result == OwnerAccessResult.denied) return false;
    if (result == OwnerAccessResult.confirmInApp) {
      final confirm = confirmUnlock;
      if (confirm == null || !await confirm() || _disposed) return false;
      access.grant();
    }
    _unlockTimer?.cancel();
    _unlockTimer = Timer(OwnerAccess.window, _relock);
    notifyListeners();
    return true;
  }

  void _relock() {
    _unlockTimer = null;
    _secretAccess?.lock();
    if (_disposed) return;
    unawaited(history.spoilers.setRevealed(null, false));
    notifyListeners();
  }

  /// The source the transform palette opens on: the focused text clip. Null
  /// while transforms are off or a module result is focused.
  Future<TransformSource?> transformSource() async {
    if (!canTransform ||
        _moduleForm != null ||
        _activating ||
        _focusedResult != null) {
      return null;
    }
    if (_tab != QuickPasteTab.history) return null;
    final clip = _focusedClip ?? items.firstOrNull;
    if (clip == null) return null;
    if (clip.secret &&
        !await _ensureSecretAccess(DeviceAuthPurpose.unlockSecrets)) {
      return null;
    }
    return TransformSource.clip(clip.id, text: clip.contentKind.isTextual);
  }

  /// Pastes a transform result the palette put on the clipboard.
  Future<PasteOutcome> pasteTransformResult() async {
    final id = _presentationId;
    if (id == null || _disposed) return PasteOutcome.noTarget;
    try {
      await _prepareAccessibilityForPaste(id);
    } catch (_) {
      return PasteOutcome.copiedOnly;
    }
    if (!_isCurrent(id) || !_accessibilityGranted) {
      return PasteOutcome.copiedOnly;
    }
    try {
      if (await _host.paste(presentationId: id)) {
        _endPresentationState();
        return PasteOutcome.pasted;
      }
      return PasteOutcome.copiedOnly;
    } on PlatformException {
      return PasteOutcome.unsupported;
    } on MissingPluginException {
      return PasteOutcome.unsupported;
    }
  }

  Future<void> _detectMaskedMode() async {
    final host = _host;
    bool? secure;
    if (host is ForegroundPasteHost) {
      try {
        secure = await (host as ForegroundPasteHost).secureFieldFocused();
      } on Object {
        secure = null;
      }
    }
    // An unknown answer (Linux) does not apply masked mode.
    _maskedMode = secure == true;
  }

  Future<void> _loadDefaultPipeline(String? targetApp) async {
    final transforms = _transforms;
    _defaultPipeline = null;
    if (transforms == null) return;
    try {
      final id = await transforms.defaultPipelineFor(
        TransformPreferenceKey.platformName(defaultTargetPlatform),
        targetApp,
      );
      if (id == null) return;
      _defaultPipeline = (await transforms.pipelines())
          .where((pipeline) => pipeline.id == id)
          .firstOrNull;
    } on Object {
      _defaultPipeline = null;
    }
  }

  Future<void> _hideInspectorFor(int presentationId) async {
    try {
      await _host.setInspectorVisible(
        presentationId: presentationId,
        visible: false,
        remember: false,
      );
    } on PlatformException {
      // The window keeps its width; Flutter shows no inspector content.
    } on MissingPluginException {
      // Hosts without an inspector need nothing hidden.
    }
  }

  Future<void> _restoreClipboard(String token) async {
    try {
      final delivery = _delivery;
      if (delivery != null) {
        await delivery.restoreClipboard(token, restoreDelay);
      } else {
        await _transforms?.restoreClipboard(token, restoreDelay);
      }
    } on Object {
      // The pasted content stays on the clipboard, the safe fallback.
    }
  }

  void _announce(QuickPasteNotice notice) {
    if (_disposed) return;
    _notice = notice;
    _noticeRevision += 1;
    notifyListeners();
  }

  /// Ends what a presentation unlocked or loaded: the secret window, the
  /// module form and the module results.
  void _endPresentationState() {
    _unlockTimer?.cancel();
    _unlockTimer = null;
    _secretAccess?.lock();
    _moduleForm = null;
    _focusedResult = null;
    _query = '';
    _resultsRequest += 1;
    _moduleItems = const {};
    _resultsLoading = false;
    _resultsFailed = false;
    _notice = null;
  }

  Future<void> activateIndex(int index) async {
    final visibleItems = history.window.items
        .where((clip) => !clip.pinned)
        .toList();
    if (index < 0 || index >= visibleItems.length) return;
    await activate(visibleItems[index]);
  }

  Future<void> toggleFocusedPin() async {
    final clip = _focusedClip;
    if (clip != null) {
      await history.mutations.togglePin(clip);
      await _reconcilePinnedShortcuts();
    }
  }

  Future<void> deleteFocused() async {
    final clip = _focusedClip;
    if (clip == null) return;
    await history.selection.select(clip.id);
    await history.mutations.deleteSelected();
  }

  Future<bool> clearUnpinned() => history.mutations.deleteAll();

  Future<void> openMainWindow() {
    _cancelInspectorHover();
    _presentationId = null;
    if (!_disposed) _endPresentationState();
    return _disposed ? Future.value() : _host.openMainWindow();
  }

  Future<void> openSettings() {
    _cancelInspectorHover();
    _presentationId = null;
    if (!_disposed) _endPresentationState();
    return _disposed ? Future.value() : _host.openSettings();
  }

  Future<void> close() async {
    _cancelInspectorHover();
    final id = _presentationId;
    _presentationId = null;
    if (!_disposed && id != null) await _closePresentation(id);
  }

  Future<void> _closePresentation(int presentationId) async {
    if (_disposed) return;
    _endPresentationState();
    try {
      await _host.close(presentationId: presentationId);
    } on PlatformException {
      // Copy remains complete if the native context has already disappeared.
    } on MissingPluginException {
      // Native teardown can race with this conditional close.
    }
  }

  Future<void> quit() {
    _cancelInspectorHover();
    _presentationId = null;
    return _disposed ? Future.value() : _host.quit();
  }

  Future<void> _refreshAccessibility() async {
    _accessibilityGranted = await _host.accessibilityGranted();
  }

  Future<void> _prepareAccessibilityForPaste(int presentationId) async {
    await _refreshAccessibility();
    if (!_isCurrent(presentationId) || _accessibilityGranted) return;
    final requested = await _preferencesStore.accessibilityPromptWasRequested();
    if (!_isCurrent(presentationId) || requested) return;
    // macOS returns before the user answers. Record the request before showing
    // it so a denial never prompts again on another selection or app launch.
    await _preferencesStore.markAccessibilityPromptRequested();
    if (!_isCurrent(presentationId)) return;
    _accessibilityGranted = await _host.requestAccessibility();
  }

  void _historyChanged() {
    if (_disposed) return;
    if (_derivedSource != null &&
        !listEquals(_derivedSource, history.window.items)) {
      _derivedSource = null;
    }
    final previousSelection = _inspectedClipId;
    final removedSelection =
        _inspectedClipId != null &&
        history.selection.selectedId == null &&
        !history.window.items.any((clip) => clip.id == _inspectedClipId);
    _inspectedClipId = history.selection.selectedId;
    final focusedId = _focusedClip?.id;
    if (focusedId != null || removedSelection) {
      final resident = history.window.items
          .where((clip) => clip.id == focusedId)
          .firstOrNull;
      if (resident != null) {
        _focusedClip = resident;
      } else if (focusedId == null || !history.window.hasVisited(focusedId)) {
        _focusedClip = null;
      }
      if (_focusedClip == null) {
        _cancelInspectorHover();
        _focusedClip = items.firstOrNull;
        final next = _focusedClip;
        if (_inspectorOpen && next != null) {
          unawaited(history.selection.select(next.id));
        }
      }
    }
    final deletedId =
        removedSelection &&
            history.mutations.isDeletePending(previousSelection!)
        ? previousSelection
        : focusedId != null &&
              history.mutations.isDeletePending(focusedId) &&
              !history.window.items.any((clip) => clip.id == focusedId)
        ? focusedId
        : null;
    final pinState = [
      for (final clip in history.window.items)
        if (clip.pinned || _pinnedShortcuts.containsKey(clip.id))
          (clip.id, clip.pinned),
    ];
    if (deletedId != null) _pendingDeletedPinIds.add(deletedId);
    if (deletedId != null || !listEquals(_scheduledPinState, pinState)) {
      _scheduledPinState = pinState;
      final deletedIds = Set<String>.of(_pendingDeletedPinIds);
      unawaited(
        _reconcilePinnedShortcuts(deletedIds: deletedIds)
            .then((_) {
              _pendingDeletedPinIds.removeAll(deletedIds);
            })
            .catchError((Object _) {
              if (identical(_scheduledPinState, pinState)) {
                _scheduledPinState = null;
              }
            }),
      );
    }
    notifyListeners();
  }

  Future<void> _reconcilePinnedShortcuts({Set<String> deletedIds = const {}}) {
    final operation = _shortcutSync.then((_) async {
      if (_disposed) return;
      final next = <String, String>{};
      for (final entry in _pinnedShortcuts.entries) {
        if (deletedIds.contains(entry.key)) continue;
        if (_availablePinKeys.containsKey(entry.value) &&
            !next.containsValue(entry.value)) {
          next[entry.key] = entry.value;
        }
      }
      for (final clip in history.window.items) {
        if (!clip.pinned) next.remove(clip.id);
      }
      for (final id in history.window.visitedPinnedIds) {
        if (next.containsKey(id)) continue;
        final available = _availablePinKeys.keys
            .where((key) => !next.containsValue(key))
            .toList();
        if (available.isEmpty) break;
        next[id] = available[Random().nextInt(available.length)];
      }
      if (mapEquals(next, _pinnedShortcuts)) return;
      await _preferencesStore.writePinnedShortcuts(next);
      if (_disposed) return;
      _pinnedShortcuts = next;
      _derivedSource = null;
      notifyListeners();
    });
    _shortcutSync = operation.then<void>((_) {}, onError: (Object error) {});
    return operation;
  }

  Future<void> shutdown() async {
    // Let an in-flight Copy settle; the native 1 s failsafe still bounds it.
    final action = _activeAction;
    dispose();
    await action;
    await _repositoryDisposal;
  }

  void _disposeHistory() {
    if (_historyDisposed) return;
    _historyDisposed = true;
    history.dispose();
  }

  @override
  void dispose() {
    if (_disposed) return;
    _cancelInspectorHover();
    _endPresentationState();
    _disposed = true;
    _presentationId = null;
    confirmUnlock = null;
    _queue?.dispose();
    _queue = null;
    _host.setOpenedHandler(null);
    _host.setShutdownHandler(null);
    history.removeListener(_historyChanged);
    if (!_activating) _disposeHistory();
    final disposeRepository = _disposeRepository;
    if (disposeRepository != null) {
      _repositoryDisposal = disposeRepository();
      unawaited(_repositoryDisposal!.catchError((Object _) {}));
    }
    unawaited(_host.dispose());
    super.dispose();
  }
}

const _numberKeys = [
  LogicalKeyboardKey.digit1,
  LogicalKeyboardKey.digit2,
  LogicalKeyboardKey.digit3,
  LogicalKeyboardKey.digit4,
  LogicalKeyboardKey.digit5,
  LogicalKeyboardKey.digit6,
  LogicalKeyboardKey.digit7,
  LogicalKeyboardKey.digit8,
  LogicalKeyboardKey.digit9,
];

// Reserve editing (select/copy/cut/paste/undo/redo), find, refresh, close,
// quit, transform and the pin action. These commands must never activate a
// pinned clip.
// macOS also owns Command-H and Command-M, including in the nonactivating popup.
const _pinKeys = {
  'b': LogicalKeyboardKey.keyB,
  'd': LogicalKeyboardKey.keyD,
  'e': LogicalKeyboardKey.keyE,
  'g': LogicalKeyboardKey.keyG,
  'i': LogicalKeyboardKey.keyI,
  'j': LogicalKeyboardKey.keyJ,
  'k': LogicalKeyboardKey.keyK,
  'l': LogicalKeyboardKey.keyL,
  'n': LogicalKeyboardKey.keyN,
  'o': LogicalKeyboardKey.keyO,
  's': LogicalKeyboardKey.keyS,
  'u': LogicalKeyboardKey.keyU,
};

const _otherPinKeys = {
  ..._pinKeys,
  'h': LogicalKeyboardKey.keyH,
  'm': LogicalKeyboardKey.keyM,
};
