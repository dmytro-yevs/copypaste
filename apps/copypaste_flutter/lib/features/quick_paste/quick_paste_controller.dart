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
import '../snippets/controller/concealed_snippet_access.dart';
import '../snippets/controller/template_fill_controller.dart';
import '../snippets/models/snippet_models.dart';
import '../snippets/repository/snippet_repository.dart';
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

enum QuickPasteTab { history, snippets, queue }

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

  /// Snippet rows shown above history results in the mixed History tab.
  static const mixedSnippetLimit = 5;
  static const snippetResultLimit = 50;

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
    SnippetRepository? snippets,
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
       _snippets = snippets,
       _transforms = transforms,
       _delivery = delivery,
       _createQueue = createQueue,
       _secretAccess = authenticator == null
           ? null
           : ConcealedSnippetAccess(authenticator: authenticator, clock: clock),
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

  final SnippetRepository? _snippets;
  final TransformRepository? _transforms;
  final TransformRecentsStore? transformRecents;
  final QuickPasteDeliveryRepository? _delivery;
  final QuickPasteQueueFactory? _createQueue;
  final ConcealedSnippetAccess? _secretAccess;
  PasteQueueController? _queue;
  QuickPasteTab _tab = QuickPasteTab.history;
  String _query = '';
  List<SnippetHit> _snippetHits = const [];
  List<SnippetSummary> _snippetLibrary = const [];
  bool _snippetsLoading = false;
  bool _snippetsFailed = false;
  int _snippetRequest = 0;
  SnippetSummary? _focusedSnippet;
  SnippetPreferences? _snippetPreferences;
  bool _snippetsUsed = false;
  TemplateFillController? _templateFill;
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
    if (_snippets != null) QuickPasteTab.snippets,
    if (_createQueue != null) QuickPasteTab.queue,
  ];

  /// Snippets above history in the History tab while searching, or the
  /// whole result list in the Snippets tab, in the runtime's ranked order.
  List<SnippetSummary> get snippetResults {
    if (_tab == QuickPasteTab.queue) return const [];
    if (_query.trim().isEmpty) {
      return _tab == QuickPasteTab.snippets ? _snippetLibrary : const [];
    }
    final rows = [for (final hit in _snippetHits) hit.snippet];
    return _tab == QuickPasteTab.history
        ? rows.take(mixedSnippetLimit).toList(growable: false)
        : rows;
  }

  bool isExactSnippet(String id) => _snippetHits.any(
    (hit) =>
        hit.snippet.id == id && hit.tier == SnippetMatchTier.exactAbbreviation,
  );

  bool get snippetsLoading => _snippetsLoading;
  bool get snippetsFailed => _snippetsFailed;
  String get query => _query;
  SnippetSummary? get focusedSnippet => _focusedSnippet;
  TemplateFillController? get templateFill => _templateFill;
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
    await Future.wait([history.initialize(), _refreshAccessibility()]);
    if (_disposed) return;
    await _reconcilePinnedShortcuts();
    if (_disposed) return;
    _initialHistoryReady = true;
    await _host.signalReady();
    if (!_disposed) notifyListeners();
  }

  Future<void> opened(
    int presentationId, {
    bool inspectorVisible = false,
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
    await Future.wait([_detectMaskedMode(), _loadDefaultPipeline()]);
    if (!_isCurrent(presentationId)) return;
    if (_maskedMode && _inspectorOpen) {
      _inspectorOpen = false;
      unawaited(_hideInspectorFor(presentationId));
    }
    if (_tab == QuickPasteTab.snippets) unawaited(_refreshSnippets());
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
    _focusedSnippet = null;
    _closeTemplateForm();
    _focusGeneration += 1;
    _pageLoad = null;
    _paginationFailed = false;
    _query = query;
    unawaited(_refreshSnippets());
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

  /// Moves through the snippet rows, then the history rows below them.
  Future<void> moveFocus(int offset) async {
    if (_tab == QuickPasteTab.queue || _templateFill != null || offset == 0) {
      return;
    }
    final rows = snippetResults;
    final snippet = _focusedSnippet;
    if (_tab == QuickPasteTab.snippets || snippet != null) {
      final index = snippet == null
          ? -1
          : rows.indexWhere((row) => row.id == snippet.id);
      final next = index < 0 ? 0 : index + offset;
      if (next >= 0 && next < rows.length) {
        focusSnippet(rows[next]);
      } else if (_tab == QuickPasteTab.snippets && rows.isNotEmpty) {
        focusSnippet(rows[next.clamp(0, rows.length - 1)]);
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
      focusSnippet(offset > 0 ? rows.first : rows.last);
      return;
    }
    await _moveHistoryFocus(offset);
  }

  void focusSnippet(SnippetSummary snippet) {
    if (_focusedSnippet?.id == snippet.id) return;
    _cancelInspectorHover();
    _focusedSnippet = snippet;
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
    if (_focusedClip?.id == clip.id && _focusedSnippet == null) return;
    _cancelInspectorHover();
    _autoOpenSuppressed = false;
    _focusedSnippet = null;
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
    if (_tab == QuickPasteTab.queue || _templateFill != null) return;
    final snippet =
        _focusedSnippet ??
        (_tab == QuickPasteTab.snippets ? snippetResults.firstOrNull : null);
    if (snippet != null) {
      await activateSnippet(snippet);
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
    _closeTemplateForm();
    _tab = tab;
    _focusedSnippet = null;
    if (tab == QuickPasteTab.queue) _ensureQueue();
    notifyListeners();
    if (tab != QuickPasteTab.queue) await _refreshSnippets();
  }

  /// Ctrl+Tab moves to the next tab and Ctrl+Shift+Tab to the previous one.
  Future<void> cycleTab(int delta) {
    final available = tabs;
    final index = available.indexOf(_tab);
    return selectTab(available[(index + delta) % available.length]);
  }

  Future<void> _refreshSnippets() async {
    final snippets = _snippets;
    if (snippets == null || _disposed) return;
    final request = ++_snippetRequest;
    final query = _query.trim();
    if (_tab == QuickPasteTab.queue ||
        (query.isEmpty && _tab != QuickPasteTab.snippets)) {
      _snippetHits = const [];
      _snippetsLoading = false;
      _snippetsFailed = false;
      notifyListeners();
      return;
    }
    // Previous rows stay visible until the new ones arrive.
    _snippetsLoading = true;
    notifyListeners();
    var failed = false;
    List<SnippetHit>? hits;
    List<SnippetSummary>? library;
    try {
      if (query.isEmpty) {
        final page = await snippets.list(
          const SnippetListScope(SnippetScopeKind.all),
          limit: snippetResultLimit,
        );
        library = [
          ...page.snippets.where((row) => row.pinned),
          ...page.snippets.where((row) => !row.pinned),
        ];
      } else {
        hits = await snippets.search(query, limit: snippetResultLimit);
      }
    } on Object {
      failed = true;
    }
    if (_disposed || request != _snippetRequest) return;
    _snippetsLoading = false;
    _snippetsFailed = failed;
    if (library != null) _snippetLibrary = library;
    if (hits != null || failed) _snippetHits = hits ?? const [];
    // One exact abbreviation is preselected; duplicates need a choice.
    final exact = _snippetHits
        .where((hit) => hit.tier == SnippetMatchTier.exactAbbreviation)
        .toList();
    if (exact.length == 1 && !exact.single.duplicate) {
      _focusedSnippet = exact.single.snippet;
      _focusedClip = null;
    } else if (_focusedSnippet case final focused?
        when !snippetResults.any((row) => row.id == focused.id)) {
      _focusedSnippet = null;
    }
    notifyListeners();
  }

  /// Uses [snippet]: a template with fields opens its form inside the popup;
  /// anything else is pasted right away.
  Future<void> activateSnippet(SnippetSummary snippet) async {
    final snippets = _snippets;
    final presentationId = _presentationId;
    if (snippets == null ||
        presentationId == null ||
        _disposed ||
        _activating ||
        snippet.deleted) {
      return;
    }
    _cancelInspectorHover();
    _activating = true;
    final actionDone = Completer<void>();
    _activeAction = actionDone.future;
    notifyListeners();
    try {
      if (snippet.concealed && !await _ensureConcealedAccess()) return;
      if (!_isCurrent(presentationId)) return;
      final detail = await snippets.get(snippet.id);
      if (!_isCurrent(presentationId)) return;
      _snippetsUsed = true;
      if (detail.fields.isNotEmpty) {
        _templateFill = TemplateFillController(
          repository: snippets,
          snippet: detail,
        );
        return;
      }
      await _deliverSnippet(presentationId, detail);
    } on SnippetException catch (error) {
      _announce(
        QuickPasteNotice(QuickPasteNoticeKind.failed, message: error.message),
      );
    } on QuickPasteDeliveryException catch (error) {
      _announce(
        QuickPasteNotice(QuickPasteNoticeKind.failed, message: error.message),
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

  /// Pastes the open template form's output once its required fields are
  /// filled; the form stays open while a field is missing.
  Future<void> submitTemplate() async {
    final form = _templateFill;
    final presentationId = _presentationId;
    if (form == null || presentationId == null || _activating) return;
    if (!form.validate()) return;
    _activating = true;
    final actionDone = Completer<void>();
    _activeAction = actionDone.future;
    notifyListeners();
    try {
      await _deliverSnippet(presentationId, form.snippet, form: form);
    } on SnippetException catch (error) {
      _announce(
        QuickPasteNotice(QuickPasteNoticeKind.failed, message: error.message),
      );
    } on QuickPasteDeliveryException catch (error) {
      _announce(
        QuickPasteNotice(QuickPasteNoticeKind.failed, message: error.message),
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

  /// Returns from the form to the results; the query is kept.
  void closeTemplate() {
    if (_templateFill == null) return;
    _closeTemplateForm();
    notifyListeners();
  }

  void _closeTemplateForm() {
    final form = _templateFill;
    _templateFill = null;
    form?.dispose();
  }

  Future<void> _deliverSnippet(
    int presentationId,
    SnippetDetail detail, {
    TemplateFillController? form,
  }) async {
    final snippets = _snippets!;
    final values = form?.values ?? const <String, String>{};
    if (_preferences.autoPaste) {
      try {
        await _prepareAccessibilityForPaste(presentationId);
      } catch (_) {
        // Without the permission the output is still copied.
      }
      if (!_isCurrent(presentationId)) return;
    }
    final paste = _preferences.autoPaste && _accessibilityGranted;
    final delivery = paste ? SnippetDelivery.paste : SnippetDelivery.copy;
    final String? token;
    final int? cursor;
    final masked = _delivery;
    if (_maskedMode && masked != null) {
      // A password target records no usage: render, then a transient write.
      final rendered = await snippets.render(
        snippetId: detail.id,
        values: values,
        clipboard: TemplateClipboardSource.host,
        finalRender: true,
      );
      if (rendered.missingRequired.isNotEmpty) {
        form?.validate();
        return;
      }
      token = await masked.deliverText(rendered.text, secret: true);
      cursor = rendered.cursorFromEnd;
    } else if (form != null) {
      final delivered = await form.submit(delivery);
      if (delivered == null) return;
      token = delivered.token;
      cursor = delivered.cursorFromEnd;
    } else {
      final delivered = await snippets.use(detail.id, delivery: delivery);
      token = delivered.token;
      cursor = delivered.cursorFromEnd;
    }
    if (!_isCurrent(presentationId)) return;
    if (!paste) {
      if (_preferences.autoPaste) {
        _closeTemplateForm();
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
      // The output stays on the clipboard for a manual paste.
    } on MissingPluginException {
      // A disappearing native context leaves the output on the clipboard.
    }
    if (!pasted) {
      // The output stays on the clipboard and is not restored.
      if (_isCurrent(presentationId)) {
        _closeTemplateForm();
        _announce(const QuickPasteNotice(QuickPasteNoticeKind.copiedOnly));
      }
      return;
    }
    final host = _host;
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
    if (token != null) unawaited(_restoreClipboard(token));
    _endPresentationState();
  }

  /// Adds the focused clip or snippet to the paste queue.
  Future<void> addFocusedToQueue() async {
    if (_createQueue == null ||
        _disposed ||
        _activating ||
        _tab == QuickPasteTab.queue ||
        _templateFill != null) {
      return;
    }
    final snippet =
        _focusedSnippet ??
        (_tab == QuickPasteTab.snippets ? snippetResults.firstOrNull : null);
    final clip = snippet == null && _tab == QuickPasteTab.history
        ? _focusedClip ?? items.firstOrNull
        : null;
    if (snippet == null && clip == null) return;
    // Secrets join the queue only after an unlock.
    if (clip != null &&
        clip.secret &&
        !await _ensureSecretAccess(DeviceAuthPurpose.unlockSecrets)) {
      return;
    }
    if (snippet != null &&
        snippet.concealed &&
        !await _ensureConcealedAccess()) {
      return;
    }
    final queue = _ensureQueue();
    if (queue == null) return;
    final added = await queue.add([
      if (snippet != null)
        PasteQueueSource.snippet(snippet.id)
      else
        PasteQueueSource.clip(clip!.id),
    ]);
    _announce(
      added
          ? QuickPasteNotice(
              QuickPasteNoticeKind.addedToQueue,
              count: queue.pendingCount,
            )
          : const QuickPasteNotice(QuickPasteNoticeKind.failed),
    );
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
  /// [ConcealedSnippetAccess.window].
  Future<bool> unlockSecrets() =>
      _ensureSecretAccess(DeviceAuthPurpose.unlockSecrets);

  Future<bool> _ensureConcealedAccess() async {
    final snippets = _snippets;
    if (snippets == null) return true;
    final preferences = _snippetPreferences ??= await snippets.preferences();
    if (!preferences.requireAuth) return true;
    return _ensureSecretAccess(DeviceAuthPurpose.useConcealedSnippet);
  }

  Future<bool> _ensureSecretAccess(DeviceAuthPurpose purpose) async {
    final access = _secretAccess;
    if (access == null || access.unlocked) return true;
    final result = await access.request(
      deviceAuthReason(AppStrings.current, purpose),
    );
    if (_disposed || result == ConcealedAccess.denied) return false;
    if (result == ConcealedAccess.confirmInApp) {
      final confirm = confirmUnlock;
      if (confirm == null || !await confirm() || _disposed) return false;
      access.grant();
    }
    _unlockTimer?.cancel();
    _unlockTimer = Timer(ConcealedSnippetAccess.window, _relock);
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

  /// The source the transform palette opens on: the focused text clip, or a
  /// literal snippet's rendered output. Null while transforms are off.
  Future<TransformSource?> transformSource() async {
    if (!canTransform || _templateFill != null || _activating) return null;
    final snippet = _focusedSnippet;
    if (snippet != null) {
      final snippets = _snippets!;
      if (snippet.template &&
          (await snippets.get(snippet.id)).fields.isNotEmpty) {
        return null;
      }
      if (snippet.concealed && !await _ensureConcealedAccess()) return null;
      _snippetsUsed = true;
      final rendered = await snippets.render(
        snippetId: snippet.id,
        clipboard: TemplateClipboardSource.host,
        finalRender: true,
      );
      return TransformSource.text(rendered.text);
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

  Future<void> _loadDefaultPipeline() async {
    final transforms = _transforms;
    _defaultPipeline = null;
    if (transforms == null) return;
    try {
      final id = (await transforms.preferences()).defaultPastePipelineId;
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
  /// template form and the snippet render cache.
  void _endPresentationState() {
    _unlockTimer?.cancel();
    _unlockTimer = null;
    _secretAccess?.lock();
    _closeTemplateForm();
    _focusedSnippet = null;
    _query = '';
    _snippetRequest += 1;
    _snippetHits = const [];
    _snippetLibrary = const [];
    _snippetsLoading = false;
    _snippetsFailed = false;
    _snippetPreferences = null;
    _notice = null;
    final snippets = _snippets;
    if (snippets != null && _snippetsUsed) {
      _snippetsUsed = false;
      unawaited(snippets.release().catchError((Object _) {}));
    }
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
