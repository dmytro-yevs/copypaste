import 'dart:async';

import 'package:flutter/foundation.dart';

import '../models/history_models.dart';
import 'history_controller.dart';
import 'history_errors.dart';
import 'history_media_cache.dart';
import 'history_refresh_coordinator.dart';
import 'history_resource_limits.dart';
import 'history_scope.dart';
import 'history_spoiler_controller.dart';
import 'history_summary.dart';
import 'history_tags_controller.dart';
import 'history_window_index.dart';

enum HistoryLoadState { initial, loading, ready, empty, error }

String _historyDateSectionKey(DateTime value, DateTime now) {
  final local = value.toLocal();
  final date = DateTime(local.year, local.month, local.day);
  final today = DateTime(now.year, now.month, now.day);
  final yesterday = DateTime(today.year, today.month, today.day - 1);
  if (date == today) return 'today';
  if (date == yesterday) return 'yesterday';
  return 'date-${date.toIso8601String().split('T').first}';
}

/// Owns the query and filters, the paged row window with its resident
/// summaries, facets and section state, and the passes that load or refresh
/// them.
class HistoryWindowController {
  HistoryWindowController(
    this._scope,
    this._history, {
    required HistoryErrors errors,
    required HistorySpoilerController spoilers,
    required HistoryTagsController tags,
    required HistoryMediaCache media,
    required HistoryRefreshCoordinator refresh,
    required Future<void> Function() ensureWatch,
    required int pageSize,
    required bool includeFacets,
    required bool separatePinnedWindow,
    required Duration searchDebounce,
  }) : _errors = errors,
       _spoilers = spoilers,
       _tags = tags,
       _media = media,
       _refresh = refresh,
       _ensureWatch = ensureWatch,
       _pageRows = pageSize.clamp(1, HistoryResourceLimits.pageRows),
       _includeFacets = includeFacets,
       _separatePinnedWindow = separatePinnedWindow,
       _searchDebounce = searchDebounce;

  final HistoryScope _scope;
  final HistoryController _history;
  final HistoryErrors _errors;
  final HistorySpoilerController _spoilers;
  final HistoryTagsController _tags;
  final HistoryMediaCache _media;
  final HistoryRefreshCoordinator _refresh;
  final Future<void> Function() _ensureWatch;
  final int _pageRows;
  final bool _includeFacets;
  final bool _separatePinnedWindow;
  final Duration _searchDebounce;

  // All row changes go through these helpers to preserve exposed snapshots.
  List<HistoryClip> _items = [];
  bool _itemsFrozen = false;

  late HistoryWindowIndex _window = HistoryWindowIndex(
    maxPages: HistoryResourceLimits.residentPages,
    maxBytes: HistoryResourceLimits.residentSummaryBytes,
    independentPins: _separatePinnedWindow,
  );

  Future<Set<String>> Function()? beforeWindowChange;
  VoidCallback? afterWindowChange;

  final Set<String> _collapsedSections = <String>{};
  final Map<String, double> _quickPastePinnedRowExtents = <String, double>{};
  final Map<String, double> _quickPasteRecentRowExtents = <String, double>{};
  Timer? _searchTimer;
  HistoryQuery _query = const HistoryQuery();
  HistoryFacets _facets = const HistoryFacets();
  HistoryLoadState _state = HistoryLoadState.initial;
  String? _nextCursor;
  int _skippedUndecryptable = 0;
  bool _isLoadingMore = false;
  int _queryEpoch = 0;

  int? get _firstResidentPage {
    final index = _window.pages.indexWhere((page) => page.items != null);
    return index < 0 ? null : index;
  }

  int? get _firstRecentResidentPage {
    final index = _window.pages.indexWhere(
      (page) => page.items != null && page.items!.any((item) => !item.pinned),
    );
    return index < 0 ? null : index;
  }

  int? get _lastResidentPage {
    final index = _window.pages.lastIndexWhere((page) => page.items != null);
    return index < 0 ? null : index;
  }

  int? get _firstScrollablePage => _separatePinnedWindow
      ? (_firstRecentResidentPage ?? _firstResidentPage)
      : _firstResidentPage;

  bool get hasEvictedPages => _window.pages.any(
    (page) => page.items == null || page.items!.length < page.ids.length,
  );

  HistoryQuery get query => _query;

  HistoryFacets get facets => _facets;

  HistoryLoadState get state => _state;

  List<HistoryClip> get items {
    if (!_itemsFrozen) {
      _items = List.unmodifiable(_items);
      _itemsFrozen = true;
    }
    return _items;
  }

  /// Page views are controller-private ledger references plus at most three
  /// resident summary pages. Callers must never retain the nullable summaries.
  List<HistoryWindowPage> get windowPages => List.unmodifiable(_window.pages);

  int get skippedUndecryptable => _skippedUndecryptable;

  bool get canLoadMore => _nextCursor != null;

  bool get isLoadingMore => _isLoadingMore;

  String? get paginationMarker => _nextCursor;

  bool get canLoadPrevious {
    final first = _firstScrollablePage;
    if (first == null || first == 0) return false;
    if (!_separatePinnedWindow || _firstRecentResidentPage == null) return true;
    final previous = _window.pages[first - 1];
    return previous.ids.length > previous.pinnedIds.length;
  }

  bool get canLoadPinnedPrevious {
    final index = _window.pages.indexWhere(
      (page) => page.items != null && page.items!.any((item) => item.pinned),
    );
    return index > 0 && _window.pages[index - 1].pinnedIds.isNotEmpty;
  }

  bool get canLoadPinnedMore {
    final index = _window.pages.lastIndexWhere(
      (page) => page.items != null && page.items!.any((item) => item.pinned),
    );
    return index >= 0 &&
        index + 1 < _window.pages.length &&
        _window.pages[index + 1].pinnedIds.isNotEmpty;
  }

  List<String> get visitedIds => List.unmodifiable(_window.ids);

  bool hasVisited(String id) =>
      _window.pages.any((page) => page.ids.contains(id));

  List<String> get visitedPinnedIds => List.unmodifiable(_window.pinnedIds);

  bool isVisitedPinned(String id) =>
      _window.pages.any((page) => page.pinnedIds.contains(id));

  List<String> get visitedRecentIds {
    final pinned = _window.pinnedIds.toSet();
    return List.unmodifiable(_window.ids.where((id) => !pinned.contains(id)));
  }

  List<String> get _visitedVisibleIds {
    final now = DateTime.now();
    final result = <String>[];
    for (final page in _window.pages) {
      for (var i = 0; i < page.ids.length; i++) {
        final id = page.ids[i];
        if (page.pinnedIds.contains(id)) {
          if (!_collapsedSections.contains('pinned')) result.add(id);
          continue;
        }
        if (_query.sort == HistorySort.relevance) {
          result.add(id);
          continue;
        }
        final section = _historyDateSectionKey(
          DateTime.fromMillisecondsSinceEpoch(page.createdAtMillis[i]),
          now,
        );
        if (!_collapsedSections.contains(section)) result.add(id);
      }
    }
    return result;
  }

  List<String> get visibleVisitedIds => List.unmodifiable(_visitedVisibleIds);

  bool get hasUnfilteredQuery =>
      !_query.hasSearch &&
      _query.kind == null &&
      !_query.pinnedOnly &&
      _query.origin == null &&
      _query.sourceApp == null &&
      !_query.hasDateFilter &&
      !_query.hasTagFilter;

  bool get hasActiveFilters =>
      !hasUnfilteredQuery || _query.sort != HistorySort.newest;

  bool isSectionCollapsed(String key) => _collapsedSections.contains(key);

  void toggleSection(String key) {
    if (!_collapsedSections.add(key)) _collapsedSections.remove(key);
    _scope.notify();
  }

  Future<void> reportPageExtent(int index, double extent) async {
    if (_scope.disposed || index < 0 || index >= _window.pages.length) return;
    final page = _window.pages[index];
    if (page.extent == extent) return;
    page.extent = extent;
    if (_window.exceedsLimits) {
      final anchors = await beforeWindowChange?.call() ?? const <String>{};
      if (_scope.disposed) return;
      _window.retainAround(index, protectedIds: anchors);
      _publishWindow();
      _scope.notify();
    }
  }

  Future<void> reportQuickPastePageExtent(
    int index,
    bool pinned,
    double extent,
  ) async {
    if (_scope.disposed || index < 0 || index >= _window.pages.length) return;
    final page = _window.pages[index];
    if (pinned) {
      page.pinnedExtent = extent;
    } else {
      page.recentExtent = extent;
    }
    if (_window.exceedsLimits) {
      final anchors = await beforeWindowChange?.call() ?? const <String>{};
      if (_scope.disposed) return;
      _window.retainAround(index, protectedIds: anchors);
      _publishWindow();
      _scope.notify();
    }
  }

  double? quickPasteRowExtent(String id, {required bool pinned}) =>
      (pinned ? _quickPastePinnedRowExtents : _quickPasteRecentRowExtents)[id];

  void reportQuickPasteRowExtent(
    String id, {
    required bool pinned,
    required double extent,
  }) {
    if (_scope.disposed || extent < 0) return;
    (pinned ? _quickPastePinnedRowExtents : _quickPasteRecentRowExtents)[id] =
        extent;
    final index = _window.pages.indexWhere((page) => page.ids.contains(id));
    if (index < 0 || !_window.exceedsLimits) return;
    _window.retainAround(index, protectedIds: {id});
    _publishWindow();
    _scope.notify();
  }

  Future<void> materializePage(int index, {bool force = false}) async {
    if (index < 0 || index >= _window.pages.length) return;
    final page = _window.pages[index];
    if ((!force && page.items != null) || _isLoadingMore) return;
    await _loadSavedPage(index);
  }

  Future<HistoryClip?> materializeId(String id) async {
    final index = _window.pages.indexWhere((page) => page.ids.contains(id));
    if (index < 0) return null;
    final page = _window.pages[index];
    await materializePage(
      index,
      force: page.items?.any((item) => item.id == id) != true,
    );
    return clipById(id);
  }

  Future<void> setDateRange(DateTime? start, DateTime? end) {
    if (start == null || end == null) {
      return updateQuery(_query.copyWith(clearDates: true));
    }
    final a = start.toLocal();
    final b = end.toLocal();
    final from = DateTime(a.year, a.month, a.day);
    final through = DateTime(b.year, b.month, b.day);
    return updateQuery(
      _query.copyWith(
        capturedFrom: from.isBefore(through) ? from : through,
        capturedBefore: from.isAfter(through)
            ? DateTime(from.year, from.month, from.day + 1)
            : DateTime(through.year, through.month, through.day + 1),
      ),
    );
  }

  Future<void> setDatePreset(int? days, {bool yesterday = false}) {
    if (days == null) return setDateRange(null, null);
    final now = DateTime.now();
    final end = DateTime(now.year, now.month, now.day - (yesterday ? 1 : 0));
    return setDateRange(DateTime(end.year, end.month, end.day - days + 1), end);
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
    if (_history.bulk.isBulkMutating) return;
    _searchTimer?.cancel();
    _history.bulk.endBulkSelection();
    if (query.sort == HistorySort.relevance && !query.hasSearch) {
      query = query.copyWith(sort: HistorySort.newest);
    }
    _query = query;
    await reload();
  }

  Future<void> resetFilters() => updateQuery(const HistoryQuery());

  Future<void> retryHistory() async {
    _history.pinnedOrder.cancelPinnedDrag();
    await reload();
  }

  Future<void> refreshFacets() async {
    if (_scope.disposed || _scope.suspended || !_includeFacets) return;
    final epoch = _scope.generation;
    try {
      final facets = await _scope.repository.facets();
      if (!_scope.disposed && !_scope.suspended && epoch == _scope.generation) {
        final changed = !_sameFacets(_facets, facets);
        if (changed) _facets = facets;
        _tags.updateCatalogue(facets.tags);
        _media.trimSourceIcons();
        if (changed) _scope.notify();
      }
    } catch (_) {
      // History remains usable when optional filter choices are unavailable.
    }
  }

  bool _sameFacets(HistoryFacets a, HistoryFacets b) =>
      listEquals(
        [for (final f in a.originDevices) (f.id, f.label, f.deviceClass)],
        [for (final f in b.originDevices) (f.id, f.label, f.deviceClass)],
      ) &&
      listEquals(
        [for (final f in a.sourceApps) (f.id, f.label, f.iconId)],
        [for (final f in b.sourceApps) (f.id, f.label, f.iconId)],
      ) &&
      listEquals(
        [for (final tag in a.tags) (tag.id, tag.name)],
        [for (final tag in b.tags) (tag.id, tag.name)],
      );

  Future<void> reloadWithFacets() async {
    await Future.wait([reload(), refreshFacets()]);
  }

  Future<void> reload() async {
    if (_scope.disposed || _scope.suspended) return;
    await _ensureWatch();
    if (_scope.disposed || _scope.suspended) return;
    if (_history.pinnedOrder.isInteracting) {
      _queryEpoch++;
      _refresh.defer();
      return;
    }
    final epoch = ++_queryEpoch;
    // This first page supersedes the queued row pass, but not its facets.
    if (_refresh.takeQueued()) unawaited(refreshFacets());
    _isLoadingMore = false;
    _nextCursor = null;
    _replaceItems([]);
    _window.clear();
    _skippedUndecryptable = 0;
    _state = HistoryLoadState.loading;
    _errors.message = null;
    _scope.notify();
    try {
      final page = await _scope.repository.query(
        query: _query,
        limit: _pageRows,
      );
      if (!isCurrent(epoch)) {
        return;
      }
      _window.append(null, page);
      _replaceItems(List.of(page.items));
      _history.bulk.prune();
      _nextCursor = page.nextCursor;
      _skippedUndecryptable = page.skippedUndecryptable;
      _state = _items.isEmpty ? HistoryLoadState.empty : HistoryLoadState.ready;
      final selectedId = _history.selection.selectedId;
      if (selectedId != null) {
        unawaited(_history.selection.select(selectedId));
      }
    } catch (_) {
      if (!isCurrent(epoch)) return;
      _state = HistoryLoadState.error;
      _errors.message = 'History could not be loaded. Try again.';
    }
    if (isCurrent(epoch)) {
      _media.trimSourceIcons();
      _scope.notify();
    }
  }

  Future<void> loadMore() async {
    while (_refresh.inFlight != null) {
      await _refresh.inFlight;
      if (_scope.disposed) return;
    }
    final cursor = _nextCursor;
    if (cursor == null ||
        _isLoadingMore ||
        _history.bulk.isBulkMutating ||
        _history.pinnedOrder.isReorderingPinned ||
        _state != HistoryLoadState.ready) {
      return;
    }
    final epoch = _queryEpoch;
    _isLoadingMore = true;
    _scope.notify();
    try {
      final page = await _scope.repository.query(
        query: _query,
        limit: _pageRows,
        cursor: cursor,
      );
      if (!isCurrent(epoch)) {
        return;
      }
      final next = _window.pages.length;
      _window.append(cursor, page);
      _skippedUndecryptable = _window.pages.fold(
        0,
        (sum, ref) => sum + ref.skippedUndecryptable,
      );
      final anchors = _window.exceedsLimits
          ? await beforeWindowChange?.call() ?? <String>{}
          : <String>{};
      if (!isCurrent(epoch)) {
        afterWindowChange?.call();
        return;
      }
      _window.retainAround(next, protectedIds: anchors);
      _publishWindow();
    } catch (_) {
      if (isCurrent(epoch)) {
        _errors.message = 'More history could not be loaded.';
      }
    } finally {
      if (isCurrent(epoch)) {
        _isLoadingMore = false;
        _scope.notify();
      }
    }
  }

  Future<void> loadPrevious() async {
    while (_refresh.inFlight != null) {
      await _refresh.inFlight;
      if (_scope.disposed) return;
    }
    final first = _firstScrollablePage;
    if (first == null || first == 0) return;
    await _loadSavedPage(first - 1);
  }

  Future<void> loadPinnedPrevious() async {
    if (!canLoadPinnedPrevious) return;
    final first = _window.pages.indexWhere(
      (page) => page.items != null && page.items!.any((item) => item.pinned),
    );
    await _loadSavedPage(first - 1);
  }

  Future<void> loadPinnedMore() async {
    if (!canLoadPinnedMore) return;
    final last = _window.pages.lastIndexWhere(
      (page) => page.items != null && page.items!.any((item) => item.pinned),
    );
    await _loadSavedPage(last + 1);
  }

  Future<void> _loadSavedPage(int index) async {
    if (_isLoadingMore ||
        _history.bulk.isBulkMutating ||
        _history.pinnedOrder.isReorderingPinned ||
        _state != HistoryLoadState.ready) {
      return;
    }
    final epoch = _queryEpoch;
    _isLoadingMore = true;
    _scope.notify();
    try {
      final page = await _scope.repository.query(
        query: _query,
        limit: _pageRows,
        cursor: _window.pages[index].inputCursor,
      );
      if (!isCurrent(epoch)) return;
      if (!_samePageBoundary(_window.pages[index], page)) {
        await _rebuildWindow(
          epoch,
          invalidFrom: index,
          freshFirstPage: index == 0 ? page : null,
        );
        return;
      }
      _window.replace(index, page);
      _skippedUndecryptable = _window.pages.fold(
        0,
        (sum, ref) => sum + ref.skippedUndecryptable,
      );
      final anchors = _window.exceedsLimits
          ? await beforeWindowChange?.call() ?? <String>{}
          : <String>{};
      if (!isCurrent(epoch)) {
        afterWindowChange?.call();
        return;
      }
      _window.retainAround(index, protectedIds: anchors);
      _publishWindow();
    } catch (_) {
      if (isCurrent(epoch)) {
        _errors.message = 'Earlier history could not be loaded.';
      }
    } finally {
      if (isCurrent(epoch)) {
        _isLoadingMore = false;
        _scope.notify();
      }
    }
  }

  Future<void> refreshVisible() async {
    if (_state != HistoryLoadState.ready) {
      await reloadWithFacets();
      return;
    }
    final epoch = ++_queryEpoch;
    _isLoadingMore = false;
    _nextCursor = null;
    var refreshed = false;
    await Future.wait([
      () async {
        try {
          await readVisibleItems(epoch);
          refreshed = true;
        } catch (_) {
          if (isCurrent(epoch)) {
            _errors.message = 'History could not be refreshed. Try again.';
          }
        }
        if (isCurrent(epoch)) _scope.notify();
      }(),
      refreshFacets(),
    ]);
    if (refreshed &&
        isCurrent(epoch) &&
        _errors.message == HistoryErrors.watchUnavailable) {
      _errors.message = null;
      _scope.notify();
    }
  }

  bool _samePageBoundary(HistoryWindowPage ref, HistoryClipPage page) {
    if (page.nextCursor != ref.nextCursor ||
        !listEquals(ref.ids, page.items.map((item) => item.id).toList())) {
      return false;
    }
    return page.items.every(
      (item) => ref.pinnedIds.contains(item.id) == item.pinned,
    );
  }

  Future<void> readVisibleItems(int epoch) async {
    final resident = <int>[
      for (var i = 0; i < _window.pages.length; i++)
        if (_window.pages[i].items != null) i,
    ];
    final pending = <(int, HistoryClipPage)>[];
    var boundaryChanged = false;
    int? invalidFrom;
    HistoryClipPage? freshFirstPage;
    var changed = false;
    for (final index in resident) {
      final ref = _window.pages[index];
      final page = await _scope.repository.query(
        query: _query,
        limit: _pageRows,
        cursor: ref.inputCursor,
      );
      if (!isCurrent(epoch)) return;
      if (!_samePageBoundary(ref, page)) {
        if (index == 0 && ref.inputCursor == null) freshFirstPage = page;
        boundaryChanged = true;
        invalidFrom = index;
        break;
      }
      final previous = {for (final item in ref.items!) item.id: item};
      final rows = <HistoryClip>[];
      for (final candidate in page.items) {
        if (ref.pinnedIds.contains(candidate.id) != candidate.pinned) {
          boundaryChanged = true;
          break;
        }
        final old = previous[candidate.id];
        if (old == null) continue; // This half of a mixed page is evicted.
        if (old.pinned != candidate.pinned) {
          boundaryChanged = true;
          break;
        }
        final item = historySummaryEquals(old, candidate) ? old : candidate;
        rows.add(item);
        if (!identical(old, item)) changed = true;
      }
      if (boundaryChanged) {
        if (index == 0 && ref.inputCursor == null) freshFirstPage = page;
        break;
      }
      pending.add((
        index,
        HistoryClipPage(
          items: rows,
          nextCursor: page.nextCursor,
          skippedUndecryptable: page.skippedUndecryptable,
        ),
      ));
    }
    if (_scope.repository is HistorySpoilerRepository) {
      await _spoilers.refresh();
      if (!isCurrent(epoch)) return;
    }
    if (boundaryChanged) {
      pending.clear();
      await _rebuildWindow(
        epoch,
        invalidFrom: invalidFrom ?? resident.first,
        freshFirstPage: freshFirstPage,
      );
      return;
    }
    for (final (index, page) in pending) {
      // Keep the complete ID metadata even if only one half of a mixed page
      // is materialized.
      _window.pages[index].items = page.items;
      _window.pages[index].skippedUndecryptable = page.skippedUndecryptable;
    }
    if (changed) {
      final anchors = await beforeWindowChange?.call() ?? <String>{};
      if (!isCurrent(epoch)) {
        afterWindowChange?.call();
        return;
      }
      _window.retainAround(resident.last, protectedIds: anchors);
      _publishWindow();
    }
    _nextCursor = _lastResidentPage == null
        ? null
        : _window.pages[_lastResidentPage!].nextCursor;
    _skippedUndecryptable = _window.pages.fold(
      0,
      (sum, page) => sum + page.skippedUndecryptable,
    );
    _state = _items.isEmpty ? HistoryLoadState.empty : HistoryLoadState.ready;
    _history.selection.refresh();
  }

  /// A changed boundary invalidates later keyset cursors. Rebuild metadata in
  /// order, retaining only the page that contains the actual visible anchor.
  Future<void> _rebuildWindow(
    int epoch, {
    int invalidFrom = 0,
    HistoryClipPage? freshFirstPage,
  }) async {
    final anchors = await beforeWindowChange?.call() ?? <String>{};
    if (!isCurrent(epoch)) {
      afterWindowChange?.call();
      return;
    }
    final wanted =
        anchors.firstOrNull ??
        _items.firstOrNull?.id ??
        _history.selection.selectedId;
    final oldIds = visitedIds;
    final previous = {for (final item in _items) item.id: item};
    final anchorRank = oldIds.indexOf(wanted ?? '');
    final ranks = <String, int>{
      for (var i = 0; i < oldIds.length; i++) oldIds[i]: i,
    };
    final rebuilt = HistoryWindowIndex(
      maxPages: HistoryResourceLimits.residentPages,
      maxBytes: HistoryResourceLimits.residentSummaryBytes,
      independentPins: _separatePinnedWindow,
    );
    final start = invalidFrom.clamp(0, _window.pages.length).toInt();
    for (var index = 0; index < start; index++) {
      rebuilt.pages.add(_window.pages[index].ledgerCopy());
    }
    String? cursor = start < _window.pages.length
        ? _window.pages[start].inputCursor
        : null;
    int? fallbackPage;
    HistoryClipPage? fallbackContents;
    var fallbackDistance = 1 << 30;
    var found = false;
    do {
      final incoming = cursor == null && start == 0 && freshFirstPage != null
          ? freshFirstPage
          : await _scope.repository.query(
              query: _query,
              limit: _pageRows,
              cursor: cursor,
            );
      freshFirstPage = null;
      final page = HistoryClipPage(
        items: [
          for (final candidate in incoming.items)
            if (previous[candidate.id] case final old?)
              historySummaryEquals(old, candidate) ? old : candidate
            else
              candidate,
        ],
        nextCursor: incoming.nextCursor,
        skippedUndecryptable: incoming.skippedUndecryptable,
      );
      if (!isCurrent(epoch)) {
        afterWindowChange?.call();
        return;
      }
      final index = rebuilt.pages.length;
      rebuilt.append(cursor, page);
      if (fallbackPage == null && page.items.isNotEmpty) {
        fallbackPage = index;
        fallbackContents = page;
      }
      if (wanted != null && page.items.any((item) => item.id == wanted)) {
        found = true;
        break;
      }
      for (final item in page.items) {
        final rank = ranks[item.id];
        if (rank == null || anchorRank < 0) continue;
        final distance =
            (rank - anchorRank).abs() * 2 + (rank < anchorRank ? 1 : 0);
        if (distance < fallbackDistance) {
          fallbackDistance = distance;
          fallbackPage = index;
          fallbackContents = page;
        }
      }
      // Fresh boundaries have no measured scroll extent. Keep their summaries
      // until layout reports geometry and reportPageExtent can safely evict them.
      cursor = page.nextCursor;
    } while (cursor != null);
    if (!found && fallbackPage != null && fallbackContents != null) {
      rebuilt.pages[fallbackPage].items = fallbackContents.items;
    }
    if (found && rebuilt.pages.last.nextCursor != null) {
      final input = rebuilt.pages.last.nextCursor;
      final page = await _scope.repository.query(
        query: _query,
        limit: _pageRows,
        cursor: input,
      );
      if (!isCurrent(epoch)) {
        afterWindowChange?.call();
        return;
      }
      rebuilt.append(input, page);
    }
    if (_separatePinnedWindow) {
      final pinned = rebuilt.pages.indexWhere(
        (page) => page.pinnedIds.isNotEmpty,
      );
      if (pinned >= 0 && rebuilt.pages[pinned].items == null) {
        final page = await _scope.repository.query(
          query: _query,
          limit: _pageRows,
          cursor: rebuilt.pages[pinned].inputCursor,
        );
        if (!isCurrent(epoch)) {
          afterWindowChange?.call();
          return;
        }
        rebuilt.replace(pinned, page);
      }
    }
    final active = rebuilt.pages.indexWhere(
      (page) =>
          page.items != null && (wanted == null || page.ids.contains(wanted)),
    );
    if (active >= 0) {
      rebuilt.retainAround(active, protectedIds: {?wanted});
    }
    _window = rebuilt;
    if (_window.pages.isNotEmpty && _window.pages.last.nextCursor == null) {
      _history.bulk.prune();
    }
    _publishWindow();
    _skippedUndecryptable = _window.pages.fold(
      0,
      (sum, page) => sum + page.skippedUndecryptable,
    );
    _state = _items.isEmpty ? HistoryLoadState.empty : HistoryLoadState.ready;
    _history.selection.refresh();
  }

  void _publishWindow() {
    _replaceItems([
      for (final page in _window.pages)
        if (page.items != null) ...page.items!,
    ]);
    _nextCursor = _window.pages.isEmpty ? null : _window.pages.last.nextCursor;
    afterWindowChange?.call();
  }

  void _mutateItems(void Function(List<HistoryClip>) mutate) {
    if (_itemsFrozen) {
      _items = List.of(_items);
      _itemsFrozen = false;
    }
    mutate(_items);
  }

  void _replaceItems(List<HistoryClip> items) {
    _items = items;
    _itemsFrozen = false;
  }

  void replace(HistoryClip clip) {
    final index = _items.indexWhere((item) => item.id == clip.id);
    if (index >= 0) {
      final summary = historySummary(clip);
      _mutateItems((items) => items[index] = summary);
      _window.replaceItem(summary);
    }
  }

  bool isCurrent(int epoch) =>
      !_scope.disposed && !_scope.suspended && epoch == _queryEpoch;

  int get queryEpoch => _queryEpoch;

  /// Cancels every pass started under the current query epoch.
  int invalidate() => ++_queryEpoch;

  /// Starts a pass that rebuilds the continuation marker.
  int restartPass() {
    final epoch = ++_queryEpoch;
    _isLoadingMore = false;
    _nextCursor = null;
    return epoch;
  }

  /// Reads loaded rows without freezing the exposed snapshot.
  Iterable<HistoryClip> get rows => _items;
  HistoryClip? clipById(String id) =>
      _items.where((item) => item.id == id).firstOrNull;
  Iterable<String> get ledgerIds => _window.ids;
  Iterable<String> get ledgerPinnedIds => _window.pinnedIds;

  void cancelSearch() => _searchTimer?.cancel();

  void removeClip(String id) {
    _mutateItems((items) => items.removeWhere((item) => item.id == id));
    _window.removeId(id);
  }

  void updateEmptyState() {
    if (_items.isEmpty) _state = HistoryLoadState.empty;
  }

  void keepPinnedOnly() {
    _mutateItems((items) => items.removeWhere((item) => !item.pinned));
    _window.retainPinnedOnly();
    _nextCursor = null;
    _state = _items.isEmpty ? HistoryLoadState.empty : HistoryLoadState.ready;
  }

  void applyPinnedOrder(List<String> ids) {
    final byId = {
      for (final item in _items.where((item) => item.pinned)) item.id: item,
    };
    final unpinned = _items.where((item) => !item.pinned).toList();
    _replaceItems([
      ...ids.map((id) => byId[id]).whereType<HistoryClip>(),
      ...unpinned,
    ]);
  }

  void restorePinnedOrder(List<String> ids, String? nextCursor) {
    applyPinnedOrder(ids);
    _nextCursor = nextCursor;
  }

  void suspend() {
    _queryEpoch++;
    _replaceItems([]);
    _window.clear();
    _facets = const HistoryFacets();
    _nextCursor = null;
    _isLoadingMore = false;
    _state = HistoryLoadState.initial;
  }

  void dispose() {
    _queryEpoch++;
    _searchTimer?.cancel();
  }
}
