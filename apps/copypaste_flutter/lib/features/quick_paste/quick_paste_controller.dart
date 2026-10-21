import 'dart:async';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../../platform/desktop/quick_paste_host.dart';
import '../history/controller/history_controller.dart';
import '../transfers/file_operations_controller.dart';
import '../history/controller/history_ocr_controller.dart';
import '../history/models/history_models.dart';
import '../history/repository/history_file_downloader.dart';
import '../history/repository/history_file_transfer.dart';
import '../history/repository/history_repository.dart';
import '../settings/repository/quick_paste_preferences_store.dart';

class QuickPasteController extends ChangeNotifier {
  static const _inspectorHoverDelay = Duration(milliseconds: 1500);

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
  }) : _disposeRepository = disposeRepository,
       _initializeInspector = initializeInspector,
       _preferencesStore = preferencesStore,
       _host = host,
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
  Future<void>? _pageLoad;
  bool _paginationFailed = false;
  int _focusGeneration = 0;
  Timer? _inspectorHoverTimer;
  String? _hoveredClipId;
  bool _autoOpenSuppressed = false;
  bool? _requestedInspectorVisible;
  Future<void> _inspectorChanges = Future<void>.value();

  bool get autoPaste => _preferences.autoPaste;
  bool get accessibilityGranted => _accessibilityGranted;
  bool get activating => _activating;
  double? transferProgressFor(String id) => history.downloadProgressFor(id);
  bool transferFailedFor(String id) => history.downloadFailedFor(id);
  int get presentationGeneration => _presentationGeneration;
  HistoryClip? get focusedClip => _focusedClip;
  bool get inspectorOpen => _inspectorOpen;
  bool get paginationFailed => _paginationFailed;
  int? get renderablePresentationId =>
      _host is QuickPasteFrameHost &&
          _presentationId == _renderablePresentationId &&
          history.state != HistoryLoadState.initial &&
          history.state != HistoryLoadState.loading
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
    final source = history.items;
    if (!identical(_shortcutClipQuery, history.query) ||
        history.state == HistoryLoadState.initial) {
      _shortcutClipQuery = history.query;
      _shortcutClips.clear();
    }
    _shortcutClips.removeWhere(
      (id, _) =>
          !_pinnedShortcuts.containsKey(id) || !history.isVisitedPinned(id),
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
    for (final id in history.visitedPinnedIds) {
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
    final query = history.query;
    // Native opens the first presentation after this watched page is ready.
    // Re-fetching it clears already rendered rows and duplicates startup I/O.
    final reuseInitialHistory =
        _initialHistoryReady &&
        _presentationGeneration == 0 &&
        (history.state == HistoryLoadState.ready ||
            history.state == HistoryLoadState.empty) &&
        !query.hasSearch &&
        query.kind == null &&
        !query.pinnedOnly &&
        query.origin == null &&
        query.sourceApp == null &&
        query.sort == HistorySort.newest;
    _cancelInspectorHover();
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
    if (!reuseInitialHistory) {
      await history.updateQuery(const HistoryQuery());
      if (!_isCurrent(presentationId)) return;
      await _reconcilePinnedShortcuts();
      if (!_isCurrent(presentationId)) return;
    }
    await _refreshAccessibility();
    if (_inspectorOpen && _isCurrent(presentationId) && items.isNotEmpty) {
      await history.select(items.first.id);
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
    _focusGeneration += 1;
    _pageLoad = null;
    _paginationFailed = false;
    final sort = query.trim().isEmpty
        ? HistorySort.newest
        : HistorySort.relevance;
    return history.updateQuery(HistoryQuery(search: query, sort: sort));
  }

  Future<void> loadMore() {
    if (_disposed) return Future<void>.value();
    final pending = _pageLoad;
    if (pending != null) return pending;
    final generation = _focusGeneration;
    final marker = history.paginationMarker;
    _paginationFailed = false;
    late final Future<void> operation;
    operation = history.loadMore().whenComplete(() {
      if (identical(_pageLoad, operation)) _pageLoad = null;
      if (!_disposed && generation == _focusGeneration) {
        _paginationFailed =
            history.paginationMarker == marker && history.canLoadMore;
        notifyListeners();
      }
    });
    _pageLoad = operation;
    return operation;
  }

  /// Traverses data rather than mounted rows, including page boundaries.
  Future<void> moveFocus(int offset) async {
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
          ? history.visitedPinnedIds
          : history.visitedRecentIds;
      var index = group.indexOf(current.id);
      if (offset > 0 &&
          !current.pinned &&
          index == group.length - 1 &&
          history.canLoadMore) {
        final marker = history.paginationMarker;
        await loadMore();
        if (_disposed || generation != _focusGeneration) return;
        if (history.paginationMarker == marker && history.canLoadMore) break;
        group = history.visitedRecentIds;
        index = group.indexOf(current.id);
      } else if (offset < 0 &&
          current.pinned &&
          history.canLoadPinnedPrevious &&
          visible.where((clip) => clip.pinned).firstOrNull?.id == current.id) {
        await history.loadPinnedPrevious();
      } else if (offset > 0 &&
          current.pinned &&
          history.canLoadPinnedMore &&
          visible.where((clip) => clip.pinned).lastOrNull?.id == current.id) {
        await history.loadPinnedMore();
      } else if (offset < 0 &&
          !current.pinned &&
          history.canLoadPrevious &&
          visible.where((clip) => !clip.pinned).firstOrNull?.id == current.id) {
        await history.loadPrevious();
      }
      if (_disposed || generation != _focusGeneration) return;
      visible = items;
      final nextIndex = index + offset.sign;
      if (index >= 0 && nextIndex >= 0 && nextIndex < group.length) {
        final target = group[nextIndex];
        final next =
            visible.where((clip) => clip.id == target).firstOrNull ??
            await history.materializeId(target);
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
    if (_focusedClip?.id == clip.id) return;
    _cancelInspectorHover();
    _autoOpenSuppressed = false;
    _focusedClip = clip;
    if (_inspectorOpen) unawaited(history.select(clip.id));
    notifyListeners();
  }

  void hoverClip(HistoryClip clip, {required bool hovered}) {
    if (_disposed) return;
    if (!hovered) {
      if (_hoveredClipId == clip.id) _cancelInspectorHover();
      return;
    }
    focus(clip);
    _hoveredClipId = clip.id;
    final id = _presentationId;
    if (id == null ||
        _activating ||
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
    if (id == null || _disposed || _activating) return Future<void>.value();
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
            if (clip != null) unawaited(history.select(clip.id));
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

  Future<void> activate(
    HistoryClip clip, {
    bool plainText = false,
    bool invertAutoPaste = false,
    bool forcePaste = false,
  }) async {
    final presentationId = _presentationId;
    if (_disposed ||
        _activating ||
        presentationId == null ||
        history.isDownloadPending(clip.id)) {
      return;
    }
    final generation = ++_activationGeneration;
    final focusGeneration = _focusGeneration;
    final focusedId = _focusedClip?.id;
    bool current() =>
        _isCurrent(presentationId) && generation == _activationGeneration;
    _cancelInspectorHover();
    _activating = true;
    notifyListeners();
    try {
      await history.select(clip.id);
      if (!current()) return;
      final selected = history.selectedClip;
      if (selected?.contentKind == HistoryClipKind.file &&
          selected?.file != null &&
          !selected!.file!.sourceAvailable) {
        // The runtime owns file I/O; other clipboard actions remain available.
        _activating = false;
        notifyListeners();
        final path = await history.prepareFileForCopy(selected);
        if (path == null ||
            !current() ||
            focusGeneration != _focusGeneration ||
            focusedId != _focusedClip?.id) {
          return;
        }
        _activating = true;
        await history.select(clip.id);
        if (!current()) return;
      }
      final copied = await history.copySelected(plainText: plainText);
      if (!copied || !current()) return;
      final shouldPaste = forcePaste
          ? true
          : invertAutoPaste
          ? !_preferences.autoPaste
          : _preferences.autoPaste;
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
        if (!current() || pasted) return;
      }
      await _closePresentation(presentationId);
    } finally {
      if (generation == _activationGeneration) {
        _activating = false;
      }
      if (_disposed) _disposeHistory();
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> activateFocused({
    bool plainText = false,
    bool invertAutoPaste = false,
    bool forcePaste = false,
  }) async {
    final clip = _focusedClip ?? items.firstOrNull;
    if (clip == null) return;
    await activate(
      clip,
      plainText: plainText,
      invertAutoPaste: invertAutoPaste,
      forcePaste: forcePaste,
    );
  }

  Future<void> activateIndex(int index) async {
    final visibleItems = history.items.where((clip) => !clip.pinned).toList();
    if (index < 0 || index >= visibleItems.length) return;
    await activate(visibleItems[index]);
  }

  Future<void> toggleFocusedPin() async {
    final clip = _focusedClip;
    if (clip != null) {
      await history.togglePin(clip);
      await _reconcilePinnedShortcuts();
    }
  }

  Future<void> deleteFocused() async {
    final clip = _focusedClip;
    if (clip == null) return;
    await history.select(clip.id);
    await history.deleteSelected();
  }

  Future<bool> clearUnpinned() => history.deleteAll();

  Future<void> openMainWindow() {
    _cancelInspectorHover();
    _presentationId = null;
    return _disposed ? Future.value() : _host.openMainWindow();
  }

  Future<void> openSettings() {
    _cancelInspectorHover();
    _presentationId = null;
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
    if (_derivedSource != null && !listEquals(_derivedSource, history.items)) {
      _derivedSource = null;
    }
    final previousSelection = _inspectedClipId;
    final removedSelection =
        _inspectedClipId != null &&
        history.selectedId == null &&
        !history.items.any((clip) => clip.id == _inspectedClipId);
    _inspectedClipId = history.selectedId;
    final focusedId = _focusedClip?.id;
    if (focusedId != null || removedSelection) {
      final resident = history.items
          .where((clip) => clip.id == focusedId)
          .firstOrNull;
      if (resident != null) {
        _focusedClip = resident;
      } else if (focusedId == null || !history.hasVisited(focusedId)) {
        _focusedClip = null;
      }
      if (_focusedClip == null) {
        _cancelInspectorHover();
        _focusedClip = items.firstOrNull;
        final next = _focusedClip;
        if (_inspectorOpen && next != null) unawaited(history.select(next.id));
      }
    }
    final deletedId =
        removedSelection && history.isDeletePending(previousSelection!)
        ? previousSelection
        : focusedId != null &&
              history.isDeletePending(focusedId) &&
              !history.items.any((clip) => clip.id == focusedId)
        ? focusedId
        : null;
    final pinState = [
      for (final clip in history.items)
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
      for (final clip in history.items) {
        if (!clip.pinned) next.remove(clip.id);
      }
      for (final id in history.visitedPinnedIds) {
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

  Future<void> shutdown() {
    dispose();
    return _repositoryDisposal ?? Future<void>.value();
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
    _disposed = true;
    _presentationId = null;
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
// quit, and the pin action. These commands must never activate a pinned clip.
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
  't': LogicalKeyboardKey.keyT,
  'u': LogicalKeyboardKey.keyU,
};

const _otherPinKeys = {
  ..._pinKeys,
  'h': LogicalKeyboardKey.keyH,
  'm': LogicalKeyboardKey.keyM,
};
