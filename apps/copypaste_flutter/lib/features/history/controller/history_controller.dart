import 'dart:async';
import 'dart:collection';

import 'package:flutter/foundation.dart';

import '../models/history_models.dart';
import '../repository/history_file_downloader.dart';
import '../repository/history_repository.dart';
import '../repository/history_file_importer.dart';
import 'history_ocr_controller.dart';

enum HistoryLoadState { initial, loading, ready, empty, error }

enum HistoryFileDownloadResult { saved, cancelled, failed }

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
    ocr?.addListener(notifyListeners);
  }

  final HistoryOcrController? ocr;

  static const int _maxCachedMedia = 48;

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
  final LinkedHashMap<String, HistoryImagePreview?> _imagePreviews =
      LinkedHashMap();
  final LinkedHashMap<String, HistorySourceAppIcon?> _sourceIcons =
      LinkedHashMap();
  final Map<String, Future<HistoryImagePreview?>> _imagePreviewLoads = {};
  final Map<String, Future<HistorySourceAppIcon?>> _sourceIconLoads = {};
  final Set<String> _pinMutations = <String>{};
  final Set<String> _deleteMutations = <String>{};
  final Set<String> _downloadMutations = <String>{};
  final Set<String> _collapsedSections = <String>{};

  StreamSubscription<HistoryRuntimeEvent>? _watchSubscription;
  Timer? _searchTimer;
  HistoryQuery _query = const HistoryQuery();
  HistoryFacets _facets = const HistoryFacets();
  HistoryLoadState _state = HistoryLoadState.initial;
  String? _nextCursor;
  String? _selectedId;
  HistoryClip? _selectedClip;
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
  String? get draggedPinnedId => _draggedPinnedId;
  bool get hasUnfilteredQuery =>
      !_query.hasSearch &&
      _query.kind == null &&
      !_query.pinnedOnly &&
      _query.origin == null &&
      _query.sourceApp == null;
  bool get canReorderPinned =>
      !_disposed &&
      hasUnfilteredQuery &&
      _state == HistoryLoadState.ready &&
      !_isReorderingPinned &&
      !_isDeletingAll &&
      _pinMutations.isEmpty &&
      _deleteMutations.isEmpty &&
      _skippedUndecryptable == 0 &&
      _items.where((item) => item.pinned).take(2).length == 2;
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

  bool get canDownloadSelected {
    final file = _selectedClip?.file;
    return _fileDownloader != null && file != null && !file.sourceAvailable;
  }

  Future<void> initialize() async {
    _watchSubscription ??= _repository.watch().listen((event) {
      if (event == HistoryRuntimeEvent.itemsChanged) {
        if (_isImportingFiles) {
          _refreshAfterFileImport = true;
          return;
        }
        if (_draggedPinnedId != null || _isReorderingPinned) {
          _refreshAfterPinnedInteraction = true;
        } else {
          unawaited(_refreshItemsAndFacets());
        }
      }
    });
    await _reloadItemsAndFacets();
  }

  Future<void> _reloadItemsAndFacets() async {
    await Future.wait([reload(), refreshFacets()]);
  }

  Future<void> _refreshItemsAndFacets() {
    late final Future<void> refresh;
    refresh = _refreshVisibleItemsAndFacets().whenComplete(() {
      if (identical(_visibleItemsRefresh, refresh)) _visibleItemsRefresh = null;
    });
    _visibleItemsRefresh = refresh;
    return refresh;
  }

  Future<void> _refreshVisibleItemsAndFacets() async {
    if (_state != HistoryLoadState.ready) {
      await _reloadItemsAndFacets();
      return;
    }
    final epoch = ++_queryEpoch;
    final retainedCount = _items.length;
    _isLoadingMore = false;
    _nextCursor = null;
    await Future.wait([
      () async {
        try {
          await _readVisibleItems(epoch, retainedCount);
        } catch (_) {
          if (_isCurrent(epoch)) {
            _errorMessage = 'History could not be refreshed. Try again.';
          }
        }
        if (_isCurrent(epoch)) notifyListeners();
      }(),
      refreshFacets(),
    ]);
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
    _items
      ..clear()
      ..addAll(refreshed);
    _nextCursor = cursor;
    _skippedUndecryptable = skipped;
    _state = _items.isEmpty ? HistoryLoadState.empty : HistoryLoadState.ready;
  }

  Future<void> refreshFacets() async {
    try {
      _facets = await _repository.facets();
      if (!_disposed) {
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
      updateQuery(_query.copyWith(search: value));
    });
  }

  Future<void> updateQuery(HistoryQuery query) async {
    _searchTimer?.cancel();
    if (query.sort == HistorySort.relevance && !query.hasSearch) {
      query = query.copyWith(sort: HistorySort.newest);
    }
    _query = query;
    await reload();
  }

  Future<void> reload() async {
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
      _nextCursor = page.nextCursor;
      _skippedUndecryptable = page.skippedUndecryptable;
      _state = _items.isEmpty ? HistoryLoadState.empty : HistoryLoadState.ready;
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
    final epoch = _queryEpoch;
    _selectedId = id;
    _selectedClip = _items.where((item) => item.id == id).firstOrNull;
    notifyListeners();
    try {
      final fullClip = await _repository.get(id);
      if (!_isCurrent(epoch) || _selectedId != id) return;
      _selectedClip = fullClip;
      _replace(fullClip);
      notifyListeners();
    } catch (_) {
      if (!_isCurrent(epoch) || _selectedId != id) return;
      _errorMessage = 'This clip could not be opened.';
      notifyListeners();
    }
  }

  void clearSelection() {
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

  Future<bool> togglePin(HistoryClip clip) async {
    if (_isReorderingPinned || _draggedPinnedId != null) return false;
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
    if (_isReorderingPinned || _draggedPinnedId != null) return false;
    if (!_deleteMutations.add(id)) {
      return false;
    }
    notifyListeners();
    try {
      await _repository.delete(id);
      _items.removeWhere((item) => item.id == id);
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
    if (_isDeletingAll || _isReorderingPinned || _draggedPinnedId != null) {
      return false;
    }
    _isDeletingAll = true;
    notifyListeners();
    try {
      await _repository.deleteAll();
      _items.removeWhere((item) => !item.pinned);
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
    final cacheKey =
        '$id:${maxEdge ?? 'thumbnail'}:${bounds?.width}x${bounds?.height}';
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

  void _trimCachedMedia<T>(
    LinkedHashMap<String, T?> cache, {
    Set<String> retainedIds = const {},
  }) {
    final excess = cache.length - _maxCachedMedia;
    if (excess <= 0) return;
    // Displayed icons must not evict one another and trigger reloads on repaint.
    final retired = cache.keys
        .where((id) => !retainedIds.contains(id))
        .take(excess)
        .toList();
    for (final id in retired) {
      cache.remove(id);
    }
  }

  Future<T?> _loadCached<T>({
    required LinkedHashMap<String, T?> cache,
    required Map<String, Future<T?>> inFlight,
    required String id,
    required Future<T?> Function() loader,
    Set<String> Function()? retainedIds,
    bool queryBound = true,
  }) {
    final hasCached = cache.containsKey(id);
    final cached = cache.remove(id);
    if (hasCached) {
      cache[id] = cached;
      return SynchronousFuture<T?>(cached);
    }
    final existing = inFlight[id];
    if (existing != null) return existing;
    final epoch = _queryEpoch;
    late final Future<T?> request;
    request = () async {
      try {
        final value = await loader();
        if (_disposed || (queryBound && !_isCurrent(epoch))) {
          return null;
        }
        cache[id] = value;
        _trimCachedMedia(cache, retainedIds: retainedIds?.call() ?? const {});
        notifyListeners();
        return value;
      } catch (_) {
        if (!_disposed && (!queryBound || _isCurrent(epoch))) {
          cache[id] = null;
          _trimCachedMedia(cache, retainedIds: retainedIds?.call() ?? const {});
        }
        return null;
      } finally {
        inFlight.remove(id);
      }
    }();
    inFlight[id] = request;
    return request;
  }

  void _replace(HistoryClip clip) {
    final index = _items.indexWhere((item) => item.id == clip.id);
    if (index >= 0) {
      _items[index] = clip;
    }
  }

  bool _isCurrent(int epoch) => !_disposed && epoch == _queryEpoch;

  @override
  void dispose() {
    _disposed = true;
    ocr?.removeListener(notifyListeners);
    ocr?.dispose();
    _queryEpoch++;
    _searchTimer?.cancel();
    _watchSubscription?.cancel();
    _imagePreviewLoads.clear();
    _sourceIconLoads.clear();
    super.dispose();
  }
}
