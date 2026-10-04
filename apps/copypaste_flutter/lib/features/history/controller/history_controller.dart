import 'dart:async';
import 'dart:collection';

import 'package:flutter/foundation.dart';

import '../models/history_models.dart';
import '../repository/history_file_downloader.dart';
import '../repository/history_repository.dart';

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
  }) : _fileDownloader = fileDownloader,
       _searchDebounce = searchDebounce ?? const Duration(milliseconds: 250);

  static const int _maxCachedMedia = 48;

  final HistoryRepository _repository;
  final int pageSize;
  final Duration _searchDebounce;
  final HistoryFileDownloader? _fileDownloader;
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
  bool isPinPending(String id) => _pinMutations.contains(id);
  bool isDeletePending(String id) => _deleteMutations.contains(id);
  bool isDownloadPending(String id) => _downloadMutations.contains(id);
  bool get canDownloadSelected {
    final file = _selectedClip?.file;
    return _fileDownloader != null && file != null && !file.sourceAvailable;
  }

  Future<void> initialize() async {
    _watchSubscription ??= _repository.watch().listen((event) {
      if (event == HistoryRuntimeEvent.itemsChanged) {
        unawaited(_reloadItemsAndFacets());
      }
    });
    await _reloadItemsAndFacets();
  }

  Future<void> _reloadItemsAndFacets() async {
    await Future.wait([reload(), refreshFacets()]);
  }

  Future<void> refreshFacets() async {
    try {
      _facets = await _repository.facets();
      if (!_disposed) {
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
    if (_isCurrent(epoch)) notifyListeners();
  }

  Future<void> loadMore() async {
    final cursor = _nextCursor;
    if (cursor == null || _isLoadingMore || _state != HistoryLoadState.ready) {
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

  Future<bool> togglePin(HistoryClip clip) async {
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

  Future<bool> deleteSelected() async {
    final id = _selectedId;
    if (id == null) {
      return false;
    }
    if (!_deleteMutations.add(id)) {
      return false;
    }
    notifyListeners();
    try {
      await _repository.delete(id);
      _items.removeWhere((item) => item.id == id);
      clearSelection();
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
    if (_isDeletingAll) {
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

  Future<HistoryImagePreview?> requestImagePreview(String id, {int? maxEdge}) {
    final cacheKey = '$id:${maxEdge ?? 'thumbnail'}';
    return _loadCached(
      cache: _imagePreviews,
      inFlight: _imagePreviewLoads,
      id: cacheKey,
      loader: () => _repository.imagePreview(id, maxEdge: maxEdge),
    );
  }

  Future<HistorySourceAppIcon?> requestSourceIcon(String id) => _loadCached(
    cache: _sourceIcons,
    inFlight: _sourceIconLoads,
    id: id,
    loader: () => _repository.sourceAppIcon(id),
  );

  Future<T?> _loadCached<T>({
    required LinkedHashMap<String, T?> cache,
    required Map<String, Future<T?>> inFlight,
    required String id,
    required Future<T?> Function() loader,
  }) {
    final hasCached = cache.containsKey(id);
    final cached = cache.remove(id);
    if (hasCached) {
      cache[id] = cached;
      return Future<T?>.value(cached);
    }
    final existing = inFlight[id];
    if (existing != null) return existing;
    final epoch = _queryEpoch;
    late final Future<T?> request;
    request = () async {
      try {
        final value = await loader();
        if (!_isCurrent(epoch)) {
          return null;
        }
        cache[id] = value;
        while (cache.length > _maxCachedMedia) {
          cache.remove(cache.keys.first);
        }
        notifyListeners();
        return value;
      } catch (_) {
        if (_isCurrent(epoch)) {
          cache[id] = null;
          while (cache.length > _maxCachedMedia) {
            cache.remove(cache.keys.first);
          }
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
    _queryEpoch++;
    _searchTimer?.cancel();
    _watchSubscription?.cancel();
    _imagePreviewLoads.clear();
    _sourceIconLoads.clear();
    super.dispose();
  }
}
