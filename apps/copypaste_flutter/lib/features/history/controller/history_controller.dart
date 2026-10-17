import 'dart:async';
import 'dart:collection';

import 'package:flutter/foundation.dart';

import '../models/history_models.dart';
import '../repository/history_file_downloader.dart';
import '../repository/history_repository.dart';
import '../repository/history_file_importer.dart';
import 'history_ocr_controller.dart';
import 'history_resource_limits.dart';
import 'history_summary.dart';
import 'history_spoiler_controller.dart';

enum HistoryLoadState { initial, loading, ready, empty, error }

enum HistoryFileDownloadResult { saved, cancelled, failed }

enum _HistoryBulkMutation { delete, pin, unpin }

typedef _ImagePreviewKey = ({String id, int? edge, int? width, int? height});

/// Keeps paged history, selection, query cancellation and bounded media caches
/// outside the widget tree.
class HistoryController extends ChangeNotifier {
  HistoryController(
    this._repository, {
    this.pageSize = 50,
    Duration? searchDebounce,
    HistoryFileDownloader? fileDownloader,
    this.ocr,
    HistoryFilePicker? filePicker,
  }) : _filePicker = filePicker,
       _fileDownloader = fileDownloader,
       _searchDebounce = searchDebounce ?? const Duration(milliseconds: 250) {
    spoilers.addListener(notifyListeners);
    ocr?.addListener(notifyListeners);
  }

  final HistoryOcrController? ocr;
  late final spoilers = HistorySpoilerController(
    _repository is HistorySpoilerRepository
        ? _repository as HistorySpoilerRepository
        : null,
  );
  final LinkedHashMap<String, Future<HistoryClip>> _spoilerContent =
      LinkedHashMap();

  /// Loads only mounted secret content for layout beneath a nonpainting mask.
  Future<HistoryClip> spoilerContent(String id) {
    final cached = _spoilerContent.remove(id);
    final result = cached ?? _repository.get(id).then(historySummary);
    _spoilerContent[id] = result;
    while (_spoilerContent.length > _maxCachedMedia) {
      _spoilerContent.remove(_spoilerContent.keys.first);
    }
    return result;
  }

  static const int _maxCachedMedia = 48;
  static const _watchUnavailableMessage =
      'History updates are temporarily unavailable.';

  final HistoryRepository _repository;
  final int pageSize;
  final Duration _searchDebounce;
  final HistoryFileDownloader? _fileDownloader;
  final HistoryFilePicker? _filePicker;
  bool _isImportingFiles = false;
  bool _refreshAfterFileImport = false;
  bool get canImportFiles => _filePicker != null && !_disposed;
  bool get isImportingFiles => _isImportingFiles;
  final List<HistoryClip> _items = [];
  final LinkedHashMap<_ImagePreviewKey, HistoryImagePreview?> _imagePreviews =
      LinkedHashMap();
  final LinkedHashMap<String, HistorySourceAppIcon?> _sourceIcons =
      LinkedHashMap();
  final Map<_ImagePreviewKey, Future<HistoryImagePreview?>> _imagePreviewLoads =
      {};
  final Map<String, Future<HistorySourceAppIcon?>> _sourceIconLoads = {};
  final Set<String> _secretMutations = <String>{};
  final Set<String> _pinMutations = <String>{};
  final Set<String> _deleteMutations = <String>{};
  final Set<String> _downloadMutations = <String>{};
  final Set<String> _collapsedSections = <String>{};
  final Set<String> _bulkSelectedIds = <String>{};
  String? _bulkAnchorId;
  Set<String>? _bulkDragBaseline;
  bool _refreshAfterBulkDrag = false;
  bool _isBulkSelecting = false;
  bool _isBulkMutating = false;
  bool _refreshAfterBulkMutation = false;

  StreamSubscription<HistoryRuntimeEvent>? _watchSubscription;
  Future<void>? _watchCancellation;
  Timer? _watchReconnectTimer;
  int _watchGeneration = 0;
  int _watchReconnectAttempts = 0;
  Timer? _searchTimer;
  HistoryQuery _query = const HistoryQuery();
  HistoryFacets _facets = const HistoryFacets();
  HistoryLoadState _state = HistoryLoadState.initial;
  String? _nextCursor;
  String? _selectedId;
  HistoryClip? _selectedClip;
  int _selectedDetailEpoch = 0;
  Future<void>? _selectedDetailLoad;
  int? _selectedDetailLoadEpoch;
  String? _errorMessage;
  int _skippedUndecryptable = 0;
  bool _isLoadingMore = false;
  bool _isDeletingAll = false;
  bool _isReorderingPinned = false;
  bool _refreshAfterPinnedInteraction = false;
  String? _draggedPinnedId;
  String? _cancelledPinnedId;
  Future<void>? _visibleItemsRefresh;
  int _queryEpoch = 0;
  bool _disposed = false;
  bool _suspended = false;
  int _mediaEpoch = 0;

  bool get canSuspend =>
      !isBulkDragSelecting &&
      !_isBulkMutating &&
      !_isImportingFiles &&
      !_isReorderingPinned &&
      !_isDeletingAll &&
      _draggedPinnedId == null &&
      _secretMutations.isEmpty &&
      _pinMutations.isEmpty &&
      _deleteMutations.isEmpty &&
      _downloadMutations.isEmpty &&
      !(ocr?.busy ?? false);

  /// Releases loaded payloads while retaining the query, section state, and
  /// selection identity.
  void suspend() {
    if (_disposed || _suspended) return;
    _suspended = true;
    unawaited(spoilers.setRevealed(null, false));
    _isBulkSelecting = false;
    _bulkSelectedIds.clear();
    _bulkAnchorId = null;
    _bulkDragBaseline = null;
    _queryEpoch++;
    _mediaEpoch++;
    _items.clear();
    _facets = const HistoryFacets();
    _selectedClip = null;
    _selectedDetailEpoch++;
    _selectedDetailLoad = null;
    _selectedDetailLoadEpoch = null;
    _nextCursor = null;
    _isLoadingMore = false;
    _visibleItemsRefresh = null;
    _state = HistoryLoadState.initial;
    _spoilerContent.clear();
    _imagePreviews.clear();
    _sourceIcons.clear();
    _imagePreviewLoads.clear();
    _sourceIconLoads.clear();
    _cancelWatch();
  }

  HistoryQuery get query => _query;
  HistoryFacets get facets => _facets;
  HistoryLoadState get state => _state;
  List<HistoryClip> get items => List.unmodifiable(_items);
  String? get selectedId => _selectedId;
  HistoryClip? get selectedClip => _selectedClip;
  String? get errorMessage => _errorMessage;
  int get skippedUndecryptable => _skippedUndecryptable;
  bool get canLoadMore => _nextCursor != null;
  bool get isLoadingMore => _isLoadingMore;
  bool get isDeletingAll => _isDeletingAll;
  bool get isReorderingPinned => _isReorderingPinned;
  bool get isBulkSelecting => _isBulkSelecting;
  bool get isBulkMutating => _isBulkMutating;
  bool get isBulkDragSelecting => _bulkDragBaseline != null;
  Set<String> get bulkSelectedIds => Set.unmodifiable(_bulkSelectedIds);
  bool isBulkSelected(String id) => _bulkSelectedIds.contains(id);
  bool get canBeginBulkSelection =>
      !_disposed &&
      !_suspended &&
      _state == HistoryLoadState.ready &&
      !_isImportingFiles &&
      !_isDeletingAll &&
      !_isReorderingPinned &&
      !_isBulkMutating &&
      _draggedPinnedId == null &&
      _secretMutations.isEmpty &&
      _pinMutations.isEmpty &&
      _deleteMutations.isEmpty;
  String? get draggedPinnedId => _draggedPinnedId;
  bool get hasUnfilteredQuery =>
      !_query.hasSearch &&
      _query.kind == null &&
      !_query.pinnedOnly &&
      _query.origin == null &&
      _query.sourceApp == null;
  bool get canReorderPinned =>
      !_disposed &&
      !_isBulkSelecting &&
      hasUnfilteredQuery &&
      _state == HistoryLoadState.ready &&
      !_isReorderingPinned &&
      !_isDeletingAll &&
      _secretMutations.isEmpty &&
      _pinMutations.isEmpty &&
      _deleteMutations.isEmpty &&
      _skippedUndecryptable == 0 &&
      _items.where((item) => item.pinned).take(2).length == 2;
  bool isSecretPending(String id) => _secretMutations.contains(id);
  bool isPinPending(String id) =>
      _isReorderingPinned || _pinMutations.contains(id);
  bool isDeletePending(String id) =>
      _isReorderingPinned || _deleteMutations.contains(id);
  bool isDownloadPending(String id) => _downloadMutations.contains(id);
  bool isSectionCollapsed(String key) => _collapsedSections.contains(key);

  void toggleSection(String key) {
    if (!_collapsedSections.add(key)) _collapsedSections.remove(key);
    notifyListeners();
  }

  void beginBulkSelection([String? id]) {
    if (!canBeginBulkSelection) return;
    if (id != null && !_items.any((clip) => clip.id == id)) return;
    _searchTimer?.cancel();
    _isBulkSelecting = true;
    _selectedId = null;
    _selectedClip = null;
    if (id != null) {
      _bulkSelectedIds.add(id);
      _bulkAnchorId = id;
    }
    notifyListeners();
  }

  void toggleBulkSelection(String id) {
    if (!_isBulkSelecting ||
        _isBulkMutating ||
        !_items.any((clip) => clip.id == id)) {
      return;
    }
    if (!_bulkSelectedIds.add(id)) _bulkSelectedIds.remove(id);
    _bulkAnchorId = id;
    notifyListeners();
  }

  /// Range order comes from the rendered list, including its active section
  /// expansion and sort order, rather than the repository's page order.
  void selectBulkClip(
    String id, {
    required List<String> orderedIds,
    bool range = false,
    bool additive = false,
  }) {
    if (_disposed || _isBulkMutating || !orderedIds.contains(id)) return;
    final anchor = _bulkAnchorId ?? _selectedId ?? id;
    final previousSingle = _selectedId;
    if (!_isBulkSelecting) {
      beginBulkSelection();
      if (!_isBulkSelecting) return;
      if (additive &&
          previousSingle != null &&
          orderedIds.contains(previousSingle)) {
        _bulkSelectedIds.add(previousSingle);
      }
    }
    if (!range) {
      toggleBulkSelection(id);
      return;
    }
    final ids = _bulkRange(anchor, id, orderedIds);
    if (!additive) _bulkSelectedIds.clear();
    _bulkSelectedIds.addAll(ids);
    _bulkAnchorId = anchor;
    notifyListeners();
  }

  List<String> _bulkRange(String anchor, String id, List<String> orderedIds) {
    var start = orderedIds.indexOf(anchor);
    final end = orderedIds.indexOf(id);
    if (end < 0) return const [];
    if (start < 0) start = end;
    final loaded = _items.map((clip) => clip.id).toSet();
    return orderedIds
        .sublist(start < end ? start : end, (start > end ? start : end) + 1)
        .where(loaded.contains)
        .toList(growable: false);
  }

  bool beginBulkDragSelection(String id) {
    if (!canBeginBulkSelection || !_items.any((clip) => clip.id == id)) {
      return false;
    }
    if (_visibleItemsRefresh != null) {
      _queryEpoch++;
      _refreshAfterBulkDrag = true;
    }
    final baseline = Set<String>.of(_bulkSelectedIds);
    beginBulkSelection(id);
    if (!_isBulkSelecting || _bulkAnchorId != id) return false;
    _bulkDragBaseline = baseline;
    notifyListeners();
    return true;
  }

  void updateBulkDragSelection(String id, {required List<String> orderedIds}) {
    final baseline = _bulkDragBaseline;
    final anchor = _bulkAnchorId;
    if (_disposed || baseline == null || anchor == null || _isBulkMutating) {
      return;
    }
    final ids = {...baseline, ..._bulkRange(anchor, id, orderedIds)};
    if (setEquals(ids, _bulkSelectedIds)) return;
    _bulkSelectedIds
      ..clear()
      ..addAll(ids);
    notifyListeners();
  }

  void endBulkDragSelection() {
    if (_disposed || _bulkDragBaseline == null) return;
    _bulkDragBaseline = null;
    notifyListeners();
    if (_refreshAfterBulkDrag) {
      _refreshAfterBulkDrag = false;
      unawaited(_refreshItemsAndFacets());
    }
  }

  void endBulkSelection() {
    if (_disposed || _isBulkMutating) return;
    endBulkDragSelection();
    _isBulkSelecting = false;
    _bulkSelectedIds.clear();
    _bulkAnchorId = null;
    notifyListeners();
  }

  Future<bool> deleteBulkSelection() =>
      _mutateBulkSelection(_HistoryBulkMutation.delete);

  Future<bool> setBulkPinned(bool pinned) => _mutateBulkSelection(
    pinned ? _HistoryBulkMutation.pin : _HistoryBulkMutation.unpin,
  );

  Future<bool> _mutateBulkSelection(_HistoryBulkMutation action) async {
    if (!_isBulkSelecting ||
        _isBulkMutating ||
        isBulkDragSelecting ||
        _bulkSelectedIds.isEmpty) {
      return false;
    }
    _isBulkMutating = true;
    _errorMessage = null;
    notifyListeners();
    var failed = 0;
    try {
      // Finish an existing live refresh before taking the operation snapshot.
      while (_visibleItemsRefresh != null) {
        await _visibleItemsRefresh;
      }
      final clips = _items
          .where((clip) => _bulkSelectedIds.contains(clip.id))
          .toList();
      for (final clip in clips) {
        if (_disposed) return false;
        try {
          if (action == _HistoryBulkMutation.delete) {
            await _repository.delete(clip.id);
            if (_disposed) return false;
            _items.removeWhere((item) => item.id == clip.id);
            _bulkSelectedIds.remove(clip.id);
          } else {
            final pinned = action == _HistoryBulkMutation.pin;
            if (clip.pinned == pinned) continue;
            await _repository.setPinned(clip.id, pinned);
            if (_disposed) return false;
            _replace(clip.copyWith(pinned: pinned));
          }
        } catch (_) {
          failed++;
        }
      }
      if (_items.isEmpty) _state = HistoryLoadState.empty;
      _trimCachedMedia(_sourceIcons, retainedIds: _retainedSourceIconIds);
      if (action == _HistoryBulkMutation.delete && _bulkSelectedIds.isEmpty) {
        _isBulkSelecting = false;
      }
    } finally {
      if (!_disposed) {
        // Mutations can change both the pin order and pagination cursors.
        try {
          _refreshAfterBulkMutation = false;
          await _refreshItemsAndFacets();
          if (_refreshAfterBulkMutation) {
            _refreshAfterBulkMutation = false;
            await _refreshItemsAndFacets();
          }
        } finally {
          _isBulkMutating = false;
          if (failed > 0) {
            _errorMessage = action == _HistoryBulkMutation.delete
                ? 'Some selected clips could not be deleted. Try again.'
                : 'Some selected pins could not be updated. Try again.';
          }
          if (!_disposed) notifyListeners();
        }
      }
    }
    return failed == 0;
  }

  void _pruneBulkSelection() {
    final ids = _items.map((clip) => clip.id).toSet();
    _bulkSelectedIds.removeWhere((id) => !ids.contains(id));
    if (!ids.contains(_bulkAnchorId)) _bulkAnchorId = null;
  }

  bool get canDownloadSelected {
    final file = _selectedClip?.file;
    return _fileDownloader != null && file != null && !file.sourceAvailable;
  }

  Future<void> initialize() async {
    if (_disposed) return;
    _suspended = false;
    spoilers.disconnected();
    await _ensureWatch(resetBackoff: true);
    await _reloadItemsAndFacets();
    await spoilers.refresh();
    await _awaitCurrentSelectedDetail();
  }

  Future<bool> _ensureWatch({bool resetBackoff = false}) async {
    if (resetBackoff) {
      _watchReconnectAttempts = 0;
      _watchReconnectTimer?.cancel();
      _watchReconnectTimer = null;
    }
    await _watchCancellation;
    if (_disposed || _suspended) return false;
    if (_watchSubscription != null) return true;
    final generation = ++_watchGeneration;
    try {
      _watchSubscription = _repository.watch().listen(
        _onWatchEvent,
        onError: (Object error, StackTrace stackTrace) =>
            _watchErrored(generation),
        onDone: () => _watchEnded(generation),
      );
      return true;
    } catch (_) {
      _watchEnded(generation);
      return false;
    }
  }

  void _onWatchEvent(HistoryRuntimeEvent event) {
    if (_suspended || _disposed) return;
    if (event == HistoryRuntimeEvent.unavailable) {
      _watchErrored(_watchGeneration);
      return;
    }
    unawaited(spoilers.refresh());
    _watchReconnectAttempts = 0;
    if (_isBulkMutating) {
      _refreshAfterBulkMutation = true;
      return;
    }
    if (isBulkDragSelecting) {
      _refreshAfterBulkDrag = true;
      return;
    }
    if (_isImportingFiles) {
      _refreshAfterFileImport = true;
      _refreshSelectedDetail();
      return;
    }
    if (_draggedPinnedId != null || _isReorderingPinned) {
      _refreshAfterPinnedInteraction = true;
      _refreshSelectedDetail();
    } else {
      unawaited(_refreshItemsAndFacets());
    }
  }

  void _refreshSelectedDetail() {
    final id = _selectedId;
    if (id == null) return;
    final queryEpoch = _queryEpoch;
    final detailEpoch = ++_selectedDetailEpoch;
    _selectedClip = null;
    _selectedDetailLoad = null;
    _selectedDetailLoadEpoch = null;
    notifyListeners();
    unawaited(_startSelectedDetailLoad(id, queryEpoch, detailEpoch));
  }

  void _watchEnded(int generation) {
    if (generation != _watchGeneration) return;
    _watchSubscription = null;
    _watchUnavailable();
  }

  void _watchErrored(int generation) {
    if (generation != _watchGeneration) return;
    _cancelWatch();
    _watchUnavailable();
  }

  void _watchUnavailable() {
    if (_disposed || _suspended) return;
    spoilers.disconnected();
    _errorMessage = _watchUnavailableMessage;
    notifyListeners();
    _scheduleWatchReconnect();
  }

  void _scheduleWatchReconnect() {
    if (_watchReconnectTimer != null) return;
    final generation = _watchGeneration;
    final delay = switch (_watchReconnectAttempts++) {
      0 => const Duration(milliseconds: 100),
      1 => const Duration(milliseconds: 500),
      _ => const Duration(seconds: 2),
    };
    _watchReconnectTimer = Timer(delay, () {
      _watchReconnectTimer = null;
      if (_disposed || _suspended || generation != _watchGeneration) return;
      unawaited(_reconnectWatch());
    });
  }

  Future<void> _reconnectWatch() async {
    if (!await _ensureWatch()) return;
    if (_disposed || _suspended) return;
    await _refreshItemsAndFacets(resetWatchBackoff: false);
  }

  void _cancelWatch() {
    _watchGeneration++;
    _watchReconnectTimer?.cancel();
    _watchReconnectTimer = null;
    final subscription = _watchSubscription;
    _watchSubscription = null;
    if (subscription == null) return;
    late final Future<void> cancellation;
    cancellation = subscription.cancel().catchError((Object _) {}).whenComplete(
      () {
        if (identical(_watchCancellation, cancellation)) {
          _watchCancellation = null;
        }
      },
    );
    _watchCancellation = cancellation;
    unawaited(cancellation);
  }

  Future<void> _reloadItemsAndFacets({bool resetWatchBackoff = true}) async {
    await Future.wait([
      reload(resetWatchBackoff: resetWatchBackoff),
      refreshFacets(),
    ]);
  }

  Future<void> _refreshItemsAndFacets({bool resetWatchBackoff = true}) {
    late final Future<void> refresh;
    refresh =
        _refreshVisibleItemsAndFacets(
          resetWatchBackoff: resetWatchBackoff,
        ).whenComplete(() {
          if (identical(_visibleItemsRefresh, refresh)) {
            _visibleItemsRefresh = null;
          }
        });
    _visibleItemsRefresh = refresh;
    return refresh;
  }

  Future<void> _refreshVisibleItemsAndFacets({
    bool resetWatchBackoff = true,
  }) async {
    if (_state != HistoryLoadState.ready) {
      await _reloadItemsAndFacets(resetWatchBackoff: resetWatchBackoff);
      return;
    }
    final epoch = ++_queryEpoch;
    final retainedCount = _items.length;
    _isLoadingMore = false;
    _nextCursor = null;
    var refreshed = false;
    await Future.wait([
      () async {
        try {
          await _readVisibleItems(epoch, retainedCount);
          refreshed = true;
        } catch (_) {
          if (_isCurrent(epoch)) {
            _errorMessage = 'History could not be refreshed. Try again.';
          }
        }
        if (_isCurrent(epoch)) notifyListeners();
      }(),
      refreshFacets(),
    ]);
    if (refreshed &&
        _isCurrent(epoch) &&
        _errorMessage == _watchUnavailableMessage) {
      _errorMessage = null;
      notifyListeners();
    }
  }

  Future<void> _readVisibleItems(int epoch, int retainedCount) async {
    final refreshed = <HistoryClip>[];
    String? cursor;
    var skipped = 0;
    do {
      final page = await _repository.query(
        query: _query,
        limit: pageSize,
        cursor: cursor,
      );
      if (!_isCurrent(epoch)) return;
      refreshed.addAll(page.items);
      skipped += page.skippedUndecryptable;
      cursor = page.nextCursor;
    } while (cursor != null && refreshed.length < retainedCount);
    if (_repository is HistorySpoilerRepository) {
      await spoilers.refresh();
      if (!_isCurrent(epoch)) return;
    }
    _items
      ..clear()
      ..addAll(refreshed);
    _pruneBulkSelection();
    _nextCursor = cursor;
    _skippedUndecryptable = skipped;
    _state = _items.isEmpty ? HistoryLoadState.empty : HistoryLoadState.ready;
    final selectedId = _selectedId;
    if (selectedId != null) {
      unawaited(select(selectedId));
    }
  }

  Future<void> refreshFacets() async {
    if (_disposed || _suspended) return;
    final epoch = _mediaEpoch;
    try {
      final facets = await _repository.facets();
      if (!_disposed && !_suspended && epoch == _mediaEpoch) {
        _facets = facets;
        _trimCachedMedia(_sourceIcons, retainedIds: _retainedSourceIconIds);
        notifyListeners();
      }
    } catch (_) {
      // History remains usable when optional filter choices are unavailable.
    }
  }

  void updateSearch(String value) {
    _searchTimer?.cancel();
    _searchTimer = Timer(_searchDebounce, () {
      final beginsSearch = !_query.hasSearch && value.trim().isNotEmpty;
      updateQuery(
        _query.copyWith(
          search: value,
          sort: beginsSearch ? HistorySort.relevance : _query.sort,
        ),
      );
    });
  }

  Future<void> updateQuery(HistoryQuery query) async {
    if (_isBulkMutating) return;
    _searchTimer?.cancel();
    endBulkSelection();
    if (query.sort == HistorySort.relevance && !query.hasSearch) {
      query = query.copyWith(sort: HistorySort.newest);
    }
    _query = query;
    await reload();
  }

  Future<void> reload({bool resetWatchBackoff = true}) async {
    if (_disposed || _suspended) return;
    await _ensureWatch(resetBackoff: resetWatchBackoff);
    if (_disposed || _suspended) return;
    if (_draggedPinnedId != null || _isReorderingPinned) {
      _queryEpoch++;
      _refreshAfterPinnedInteraction = true;
      return;
    }
    final epoch = ++_queryEpoch;
    _isLoadingMore = false;
    _nextCursor = null;
    _items.clear();
    _skippedUndecryptable = 0;
    _state = HistoryLoadState.loading;
    _errorMessage = null;
    notifyListeners();
    try {
      final page = await _repository.query(query: _query, limit: pageSize);
      if (!_isCurrent(epoch)) {
        return;
      }
      _items.addAll(page.items);
      _pruneBulkSelection();
      _nextCursor = page.nextCursor;
      _skippedUndecryptable = page.skippedUndecryptable;
      _state = _items.isEmpty ? HistoryLoadState.empty : HistoryLoadState.ready;
      final selectedId = _selectedId;
      if (selectedId != null) {
        unawaited(select(selectedId));
      }
    } catch (_) {
      if (!_isCurrent(epoch)) return;
      _state = HistoryLoadState.error;
      _errorMessage = 'History could not be loaded. Try again.';
    }
    if (_isCurrent(epoch)) {
      _trimCachedMedia(_sourceIcons, retainedIds: _retainedSourceIconIds);
      notifyListeners();
    }
  }

  Future<void> loadMore() async {
    while (_visibleItemsRefresh != null) {
      await _visibleItemsRefresh;
      if (_disposed) return;
    }
    final cursor = _nextCursor;
    if (cursor == null ||
        _isLoadingMore ||
        _isBulkMutating ||
        _isReorderingPinned ||
        _state != HistoryLoadState.ready) {
      return;
    }
    final epoch = _queryEpoch;
    _isLoadingMore = true;
    notifyListeners();
    try {
      final page = await _repository.query(
        query: _query,
        limit: pageSize,
        cursor: cursor,
      );
      if (!_isCurrent(epoch)) {
        return;
      }
      _items.addAll(page.items);
      _nextCursor = page.nextCursor;
      _skippedUndecryptable += page.skippedUndecryptable;
    } catch (_) {
      if (_isCurrent(epoch)) {
        _errorMessage = 'More history could not be loaded.';
      }
    } finally {
      if (_isCurrent(epoch)) {
        _isLoadingMore = false;
        notifyListeners();
      }
    }
  }

  Future<void> select(String id) async {
    if (_isBulkSelecting) return;
    final queryEpoch = _queryEpoch;
    final detailEpoch = ++_selectedDetailEpoch;
    _selectedId = id;
    _selectedClip = _items.where((item) => item.id == id).firstOrNull;
    notifyListeners();
    await _startSelectedDetailLoad(id, queryEpoch, detailEpoch);
  }

  Future<void> _startSelectedDetailLoad(
    String id,
    int queryEpoch,
    int detailEpoch,
  ) {
    late final Future<void> load;
    load = _loadSelectedDetail(id, queryEpoch, detailEpoch).whenComplete(() {
      if (identical(_selectedDetailLoad, load)) {
        _selectedDetailLoad = null;
        _selectedDetailLoadEpoch = null;
      }
    });
    _selectedDetailLoad = load;
    _selectedDetailLoadEpoch = detailEpoch;
    return load;
  }

  Future<void> _awaitCurrentSelectedDetail() async {
    final load = _selectedDetailLoad;
    final detailEpoch = _selectedDetailLoadEpoch;
    if (load != null && detailEpoch == _selectedDetailEpoch) await load;
  }

  Future<void> _loadSelectedDetail(
    String id,
    int queryEpoch,
    int detailEpoch,
  ) async {
    try {
      final fullClip = await _repository.get(id);
      if (fullClip.secret && _repository is HistorySpoilerRepository) {
        await spoilers.refresh();
      }
      if (!_isCurrent(queryEpoch) ||
          _selectedId != id ||
          _selectedDetailEpoch != detailEpoch) {
        return;
      }
      _selectedClip = fullClip;
      _replace(fullClip);
      notifyListeners();
    } catch (_) {
      if (!_isCurrent(queryEpoch) ||
          _selectedId != id ||
          _selectedDetailEpoch != detailEpoch) {
        return;
      }
      _errorMessage = 'This clip could not be opened.';
      notifyListeners();
    }
  }

  void clearSelection() {
    _selectedDetailEpoch++;
    _selectedDetailLoad = null;
    _selectedDetailLoadEpoch = null;
    _selectedId = null;
    _selectedClip = null;
    notifyListeners();
  }

  Future<bool> copySelected({required bool plainText}) async {
    final clip = _selectedClip;
    if (clip == null) {
      return false;
    }
    try {
      if (plainText) {
        await _repository.copyPlainText(clip.id);
      } else {
        await _repository.copy(clip.id);
      }
      return true;
    } catch (_) {
      _errorMessage = plainText
          ? 'Plain text could not be copied.'
          : 'This clip could not be copied.';
      notifyListeners();
      return false;
    }
  }

  Future<HistoryFileDownloadResult> downloadSelected() async {
    final clip = _selectedClip;
    final file = clip?.file;
    final downloader = _fileDownloader;
    if (clip == null ||
        file == null ||
        downloader == null ||
        file.sourceAvailable ||
        !_downloadMutations.add(clip.id)) {
      return HistoryFileDownloadResult.failed;
    }
    notifyListeners();
    try {
      final destination = await downloader.chooseDestination(file);
      if (destination == null) return HistoryFileDownloadResult.cancelled;
      await _repository.saveFile(clip.id, destination);
      await downloader.presentSavedFile(destination, file);
      return HistoryFileDownloadResult.saved;
    } catch (_) {
      _errorMessage = 'This file could not be downloaded.';
      notifyListeners();
      return HistoryFileDownloadResult.failed;
    } finally {
      _downloadMutations.remove(clip.id);
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> chooseFiles() async {
    final picker = _filePicker;
    if (picker == null || _disposed || _isImportingFiles) return;
    _isImportingFiles = true;
    notifyListeners();
    String? failure;
    try {
      failure = await _importFiles(await picker.chooseFiles());
    } catch (_) {
      failure = 'Files could not be selected. Try again.';
    } finally {
      await _finishFileImport(failure);
    }
  }

  Future<void> importFiles(List<HistoryImportFile> files) async {
    if (_disposed || _isImportingFiles) {
      for (final file in files) {
        await _releaseImportFile(file);
      }
      return;
    }
    _isImportingFiles = true;
    notifyListeners();
    String? failure;
    try {
      failure = await _importFiles(files);
    } finally {
      await _finishFileImport(failure);
    }
  }

  Future<String?> _importFiles(List<HistoryImportFile> files) async {
    if (files.isEmpty) return null;
    var failed = 0;
    String? failedName;
    _errorMessage = null;
    for (final file in files) {
      try {
        if (!_disposed) {
          await _repository.importFile(file);
          _refreshAfterFileImport = true;
        }
      } catch (_) {
        failed++;
        failedName ??= file.name;
      } finally {
        await _releaseImportFile(file);
      }
    }
    return failed == 0
        ? null
        : '$failed of ${files.length} files could not be imported. First: $failedName. Check access and the History size limit.';
  }

  Future<void> _releaseImportFile(HistoryImportFile file) async {
    try {
      await file.dispose();
    } catch (_) {
      /* Best-effort temporary-file cleanup. */
    }
  }

  Future<void> _finishFileImport(String? failure) async {
    _isImportingFiles = false;
    if (_disposed) return;
    if (_refreshAfterFileImport) {
      _refreshAfterFileImport = false;
      if (_draggedPinnedId != null || _isReorderingPinned) {
        _refreshAfterPinnedInteraction = true;
      } else {
        await _refreshItemsAndFacets();
      }
    }
    if (_disposed) return;
    if (failure != null) _errorMessage = failure;
    notifyListeners();
  }

  Future<bool> toggleSecret(HistoryClip clip) async {
    if (_disposed || !_secretMutations.add(clip.id)) return false;
    if (_selectedId == clip.id) {
      _selectedDetailEpoch++;
      _selectedDetailLoad = null;
      _selectedDetailLoadEpoch = null;
    }
    notifyListeners();
    try {
      final updated = await _repository.setSecret(clip.id, !clip.secret);
      if (_disposed) return true;
      final concealed = spoilers.setRevealed(clip.id, false);
      _spoilerContent.remove(clip.id);
      _replace(updated);
      if (_selectedId == clip.id) {
        _selectedDetailEpoch++;
        _selectedDetailLoad = null;
        _selectedDetailLoadEpoch = null;
        _selectedClip = updated;
      }
      notifyListeners();
      await concealed;
      return true;
    } catch (_) {
      if (!_disposed) _errorMessage = 'The clip secrecy could not be updated.';
      return false;
    } finally {
      _secretMutations.remove(clip.id);
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> togglePin(HistoryClip clip) async {
    if (_isBulkMutating || _isReorderingPinned || _draggedPinnedId != null) {
      return false;
    }
    if (!_pinMutations.add(clip.id)) {
      return false;
    }
    final next = !clip.pinned;
    _replace(clip.copyWith(pinned: next));
    if (_selectedId == clip.id) {
      _selectedClip = _selectedClip?.copyWith(pinned: next);
    }
    notifyListeners();
    try {
      await _repository.setPinned(clip.id, next);
      return true;
    } catch (_) {
      _replace(clip);
      if (_selectedId == clip.id) {
        _selectedClip = _selectedClip?.copyWith(pinned: clip.pinned);
      }
      _errorMessage = 'The pin could not be updated.';
      notifyListeners();
      return false;
    } finally {
      _pinMutations.remove(clip.id);
      if (!_disposed) {
        notifyListeners();
      }
    }
  }

  void beginPinnedDrag(String id) {
    if (!canReorderPinned ||
        !_items.any((item) => item.id == id && item.pinned)) {
      return;
    }
    if (_visibleItemsRefresh != null) {
      _queryEpoch++;
      _refreshAfterPinnedInteraction = true;
    }
    _draggedPinnedId = id;
    _cancelledPinnedId = null;
    notifyListeners();
  }

  void endPinnedDrag() {
    if (_draggedPinnedId == null) return;
    _draggedPinnedId = null;
    if (!_disposed) notifyListeners();
    // Sortable calls drag-end before its accept callback starts the save.
    scheduleMicrotask(_flushPinnedRefresh);
  }

  void cancelPinnedDrag() {
    _cancelledPinnedId = _draggedPinnedId;
    endPinnedDrag();
    // Flutter may end an accepted pan on pointer cancellation. Reject that
    // gesture's drop before clearing the cancellation at the event boundary.
    scheduleMicrotask(() => _cancelledPinnedId = null);
  }

  void _flushPinnedRefresh() {
    if (_disposed || _draggedPinnedId != null || _isReorderingPinned) return;
    if (_refreshAfterPinnedInteraction) {
      _refreshAfterPinnedInteraction = false;
      unawaited(_refreshItemsAndFacets());
    }
  }

  Future<bool> shiftPinned(String id, {required bool up}) async {
    final pins = _items.where((item) => item.pinned).toList();
    final index = pins.indexWhere((item) => item.id == id);
    final target = index + (up ? -1 : 1);
    if (index < 0 || target < 0 || target >= pins.length) return false;
    return movePinned(id, pins[target].id, before: up);
  }

  /// Reorders the loaded pin prefix while the backend preserves its unseen tail.
  Future<bool> movePinned(
    String id,
    String targetId, {
    required bool before,
  }) async {
    if (!canReorderPinned || id == targetId || _cancelledPinnedId == id) {
      return false;
    }
    final previous = _items
        .where((item) => item.pinned)
        .map((item) => item.id)
        .toList();
    if (!previous.contains(id) || !previous.contains(targetId)) return false;
    final ordered = previous.where((value) => value != id).toList();
    ordered.insert(ordered.indexOf(targetId) + (before ? 0 : 1), id);
    if (listEquals(previous, ordered)) return false;

    final retainedCount = _items.length;
    final previousCursor = _nextCursor;
    final epoch = ++_queryEpoch;
    _isReorderingPinned = true;
    _isLoadingMore = false;
    _nextCursor = null;
    _errorMessage = null;
    _applyPinnedOrder(ordered);
    notifyListeners();
    try {
      await _repository.reorderPinned(ordered);
    } catch (_) {
      if (_isCurrent(epoch)) {
        _applyPinnedOrder(previous);
        _nextCursor = previousCursor;
        _errorMessage = 'The pinned order could not be saved. Try again.';
      }
      _isReorderingPinned = false;
      if (!_disposed) notifyListeners();
      _flushPinnedRefresh();
      return false;
    }

    try {
      if (_isCurrent(epoch)) {
        _refreshAfterPinnedInteraction = false;
        // Rebuild the continuation marker without clearing the visible list or
        // using a cursor whose last pin may have moved to a different position.
        await _readVisibleItems(epoch, retainedCount);
      }
    } catch (_) {
      if (_isCurrent(epoch)) {
        _errorMessage =
            'The order was saved, but history could not be refreshed.';
      }
    } finally {
      _isReorderingPinned = false;
      if (!_disposed) notifyListeners();
      _flushPinnedRefresh();
    }
    return true;
  }

  void _applyPinnedOrder(List<String> ids) {
    final byId = {
      for (final item in _items.where((item) => item.pinned)) item.id: item,
    };
    final unpinned = _items.where((item) => !item.pinned).toList();
    _items
      ..clear()
      ..addAll(ids.map((id) => byId[id]).whereType<HistoryClip>())
      ..addAll(unpinned);
  }

  Future<bool> deleteSelected() async {
    final id = _selectedId;
    return id == null ? false : deleteClip(id);
  }

  Future<bool> deleteClip(String id) async {
    if (_isBulkMutating || _isReorderingPinned || _draggedPinnedId != null) {
      return false;
    }
    if (!_deleteMutations.add(id)) {
      return false;
    }
    notifyListeners();
    try {
      await _repository.delete(id);
      _items.removeWhere((item) => item.id == id);
      _bulkSelectedIds.remove(id);
      if (_selectedId == id) clearSelection();
      _trimCachedMedia(_sourceIcons, retainedIds: _retainedSourceIconIds);
      if (_items.isEmpty) _state = HistoryLoadState.empty;
      notifyListeners();
      return true;
    } catch (_) {
      _errorMessage = 'This clip could not be deleted.';
      notifyListeners();
      return false;
    } finally {
      _deleteMutations.remove(id);
      if (!_disposed) {
        notifyListeners();
      }
    }
  }

  Future<bool> deleteAll() async {
    if (_isBulkMutating ||
        _isDeletingAll ||
        _isReorderingPinned ||
        _draggedPinnedId != null) {
      return false;
    }
    _isDeletingAll = true;
    notifyListeners();
    try {
      await _repository.deleteAll();
      _items.removeWhere((item) => !item.pinned);
      _pruneBulkSelection();
      _nextCursor = null;
      _state = _items.isEmpty ? HistoryLoadState.empty : HistoryLoadState.ready;
      if (_selectedId != null &&
          !_items.any((item) => item.id == _selectedId)) {
        clearSelection();
      }
      _trimCachedMedia(_sourceIcons, retainedIds: _retainedSourceIconIds);
      notifyListeners();
      return true;
    } catch (_) {
      _errorMessage = 'History could not be deleted.';
      notifyListeners();
      return false;
    } finally {
      _isDeletingAll = false;
      if (!_disposed) {
        notifyListeners();
      }
    }
  }

  Future<HistoryImagePreview?> requestImagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) {
    final cacheKey = (
      id: id,
      edge: maxEdge,
      width: bounds?.width,
      height: bounds?.height,
    );
    if (!_disposed && !_suspended && bounds != null) {
      final edge = (maxEdge ?? 2048).clamp(1, 2048);
      final width = bounds.width.clamp(1, edge);
      final height = bounds.height.clamp(1, edge);
      // Resizing the inspector can change the row width while height remains
      // the limiting dimension. Reuse the exact pixels in that case, avoiding
      // another native decode and a second engine image with identical content.
      // A smaller preview must never satisfy a request for higher resolution.
      for (final entry in _imagePreviews.entries) {
        final preview = entry.value;
        if (entry.key.id == id &&
            preview != null &&
            preview.width > 0 &&
            preview.height > 0 &&
            preview.width <= width &&
            preview.height <= height &&
            (preview.width == width || preview.height == height)) {
          _imagePreviews.remove(entry.key);
          _imagePreviews[entry.key] = preview;
          return SynchronousFuture(preview);
        }
      }
    }
    return _loadCached(
      cache: _imagePreviews,
      inFlight: _imagePreviewLoads,
      id: cacheKey,
      loader: () =>
          _repository.imagePreview(id, maxEdge: maxEdge, bounds: bounds),
    );
  }

  Future<HistorySourceAppIcon?> requestSourceIcon(String? iconId) {
    if (iconId == null) return SynchronousFuture(null);
    return _loadCached(
      cache: _sourceIcons,
      inFlight: _sourceIconLoads,
      id: iconId,
      loader: () => _repository.sourceAppIcon(iconId),
      retainedIds: () => _retainedSourceIconIds,
      queryBound: false,
    );
  }

  Set<String> get _retainedSourceIconIds => {
    for (final clip in _items) ?clip.sourceAppIconId,
    ?_selectedClip?.sourceAppIconId,
    for (final app in _facets.sourceApps) ?app.iconId,
  };

  void _trimCachedMedia<K, T>(
    LinkedHashMap<K, T?> cache, {
    Set<K> retainedIds = const {},
  }) {
    final maxBytes = _mediaCacheLimit(cache);
    var bytes = cache.values.fold<int>(
      0,
      (sum, value) => sum + _mediaBytes(value),
    );
    // Prefer retiring invisible entries, but visibility cannot bypass the byte
    // ceiling. Mounted images retain their own reference until disposed.
    final candidates = [
      ...cache.keys.where((id) => !retainedIds.contains(id)),
      ...cache.keys.where(retainedIds.contains),
    ];
    for (final id in candidates) {
      final overBytes = bytes > maxBytes;
      final overCount =
          cache.length > HistoryResourceLimits.retainedSourceIcons ||
          (cache.length > _maxCachedMedia && !retainedIds.contains(id));
      if (!overBytes && !overCount) continue;
      bytes -= _mediaBytes(cache.remove(id));
    }
  }

  int _mediaCacheLimit(Object cache) => identical(cache, _sourceIcons)
      ? HistoryResourceLimits.sourceIconCacheBytes
      : HistoryResourceLimits.previewCacheBytes;

  int _mediaBytes(Object? value) => switch (value) {
    HistoryImagePreview() => value.bytes.buffer.lengthInBytes,
    HistorySourceAppIcon() => value.bytes.buffer.lengthInBytes,
    _ => 0,
  };

  Future<T?> _loadCached<K, T>({
    required LinkedHashMap<K, T?> cache,
    required Map<K, Future<T?>> inFlight,
    required K id,
    required Future<T?> Function() loader,
    Set<K> Function()? retainedIds,
    bool queryBound = true,
  }) {
    if (_disposed || _suspended) return SynchronousFuture<T?>(null);
    final hasCached = cache.containsKey(id);
    final cached = cache.remove(id);
    if (hasCached) {
      cache[id] = cached;
      return SynchronousFuture<T?>(cached);
    }
    final existing = inFlight[id];
    if (existing != null) return existing;
    final epoch = _queryEpoch;
    final mediaEpoch = _mediaEpoch;
    late final Future<T?> request;
    request = () async {
      try {
        final value = await loader();
        if (_disposed ||
            _suspended ||
            mediaEpoch != _mediaEpoch ||
            (queryBound && !_isCurrent(epoch))) {
          return null;
        }
        // An oversized preview may still be displayed by its caller. It must
        // not evict every reusable thumbnail only to be evicted itself.
        if (_mediaBytes(value) <= _mediaCacheLimit(cache)) {
          cache[id] = value;
          _trimCachedMedia(cache, retainedIds: retainedIds?.call() ?? const {});
        }
        return value;
      } catch (_) {
        if (!_disposed &&
            !_suspended &&
            mediaEpoch == _mediaEpoch &&
            (!queryBound || _isCurrent(epoch))) {
          cache[id] = null;
          _trimCachedMedia(cache, retainedIds: retainedIds?.call() ?? const {});
        }
        return null;
      } finally {
        if (mediaEpoch == _mediaEpoch) inFlight.remove(id);
      }
    }();
    inFlight[id] = request;
    return request;
  }

  void _replace(HistoryClip clip) {
    final index = _items.indexWhere((item) => item.id == clip.id);
    if (index >= 0) {
      _items[index] = historySummary(clip);
    }
  }

  bool _isCurrent(int epoch) =>
      !_disposed && !_suspended && epoch == _queryEpoch;

  @override
  void dispose() {
    _disposed = true;
    _spoilerContent.clear();
    spoilers.removeListener(notifyListeners);
    spoilers.dispose();
    ocr?.removeListener(notifyListeners);
    ocr?.dispose();
    _queryEpoch++;
    _searchTimer?.cancel();
    _cancelWatch();
    _imagePreviewLoads.clear();
    _sourceIconLoads.clear();
    super.dispose();
  }
}
