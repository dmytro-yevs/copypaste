import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import '../models/snippet_models.dart';
import '../repository/snippet_repository.dart';
import 'concealed_snippet_access.dart';

enum SnippetsStatus { idle, loading, ready, failed }

/// A request to open the snippet editor, from a shortcut, the tray or the
/// Android "+" action. [snippetId] null creates a snippet.
class SnippetEditorRequest {
  const SnippetEditorRequest({this.snippetId, this.abbreviation});

  final String? snippetId;

  /// Prefills a new snippet, such as the query of an empty search.
  final String? abbreviation;
}

/// Settings → Snippets state. It loads while a view is attached and drops
/// the list, search results and change subscription when the last view
/// detaches, so a closed section holds no snippet content.
class SnippetsController extends ChangeNotifier {
  SnippetsController({required SnippetRepository repository, this.access})
    : _repository = repository;

  final SnippetRepository _repository;

  /// Gates concealed snippets while `snippets.require_auth` is on; null on
  /// engines without device authentication.
  final ConcealedSnippetAccess? access;

  SnippetRepository get repository => _repository;

  SnippetsStatus _status = SnippetsStatus.idle;
  SnippetListScope _scope = SnippetListScope.all;
  String _query = '';
  List<SnippetSummary> _rows = const [];
  List<SnippetSummary> _pinned = const [];
  String? _nextCursor;
  int _total = 0;
  bool _loadingMore = false;
  List<SnippetCollection> _collections = const [];
  SnippetPreferences _preferences = const SnippetPreferences();
  final Set<String> _selection = {};
  String? _anchor;
  bool _attentionOnly = false;
  String? _error;
  bool _busy = false;
  int _attached = 0;
  int _generation = 0;
  StreamSubscription<void>? _changes;
  bool _disposed = false;

  /// The latest editor request; the attached view opens it and clears it.
  final editorRequest = ValueNotifier<SnippetEditorRequest?>(null);

  SnippetsStatus get status => _status;
  SnippetListScope get scope => _scope;
  String get query => _query;
  bool get searching => _query.trim().isNotEmpty;
  List<SnippetCollection> get collections => _collections;
  SnippetPreferences get preferences => _preferences;
  bool get hasMore => !searching && _nextCursor != null;
  bool get loadingMore => _loadingMore;
  int get total => _total;
  String? get errorMessage => _error;
  bool get busy => _busy;
  Set<String> get selection => Set.unmodifiable(_selection);
  bool get selecting => _selection.isNotEmpty;
  bool get attentionOnly => _attentionOnly;

  /// The Pinned section above the list, shown only for all snippets.
  List<SnippetSummary> get pinned =>
      _scope == SnippetListScope.all && !searching && !_attentionOnly
      ? _pinned
      : const [];

  /// The list under the Pinned section.
  List<SnippetSummary> get rows {
    if (_attentionOnly) {
      return [
        for (final row in _merged)
          if (row.needsAttention) row,
      ];
    }
    final shown = pinned.map((row) => row.id).toSet();
    return [
      for (final row in _rows)
        if (!shown.contains(row.id)) row,
    ];
  }

  Iterable<SnippetSummary> get _merged sync* {
    final seen = <String>{};
    for (final row in [..._pinned, ..._rows]) {
      if (seen.add(row.id)) yield row;
    }
  }

  /// Loaded snippets with a duplicate abbreviation or from a conflict.
  int get attentionCount =>
      _merged.where((row) => row.needsAttention && !row.deleted).length;

  SnippetCollection? collection(String? id) =>
      _collections.where((collection) => collection.id == id).firstOrNull;

  bool collectionNameTaken(String name, {String? exceptId}) => _collections.any(
    (collection) =>
        collection.id != exceptId &&
        collection.name.toLowerCase() == name.trim().toLowerCase(),
  );

  void attach() {
    if (_attached++ > 0) return;
    _changes = _repository.changes().listen((_) => unawaited(_reload()));
    unawaited(_reload());
  }

  /// Drops loaded content and the template cache; locks concealed access.
  void detach() {
    if (_attached == 0 || --_attached > 0) return;
    unawaited(_changes?.cancel());
    _changes = null;
    _generation++;
    _rows = const [];
    _pinned = const [];
    _nextCursor = null;
    _selection.clear();
    _anchor = null;
    _status = SnippetsStatus.idle;
    access?.lock();
    unawaited(_repository.release().catchError((Object _) {}));
  }

  Future<void> retry() => _reload();

  void requestNewSnippet({String? abbreviation}) =>
      editorRequest.value = SnippetEditorRequest(abbreviation: abbreviation);

  void requestEdit(String id) =>
      editorRequest.value = SnippetEditorRequest(snippetId: id);

  Future<void> _reload({bool keepLoaded = true}) async {
    final generation = ++_generation;
    if (_status != SnippetsStatus.ready) {
      _status = SnippetsStatus.loading;
      notifyListeners();
    }
    final limit = keepLoaded
        ? _rows.length.clamp(SnippetLimits.pageSize, SnippetLimits.pageSize * 4)
        : SnippetLimits.pageSize;
    try {
      final loaded = await Future.wait<Object>([
        _repository.collections(),
        _repository.preferences(),
        if (searching)
          _repository.search(_query.trim())
        else
          _repository.list(_scope, limit: limit),
        if (_scope == SnippetListScope.all && !searching)
          _repository.list(
            SnippetListScope.pinned,
            limit: SnippetLimits.pinnedRows,
          ),
      ]);
      if (_disposed || generation != _generation) return;
      _collections = loaded[0] as List<SnippetCollection>;
      _preferences = loaded[1] as SnippetPreferences;
      if (loaded[2] case final SnippetPage page) {
        _rows = page.snippets;
        _nextCursor = page.nextCursor;
        _total = page.total;
      } else {
        _rows = [
          for (final hit in loaded[2] as List<SnippetHit>)
            if (!hit.snippet.deleted) hit.snippet,
        ];
        _nextCursor = null;
        _total = _rows.length;
      }
      _pinned = loaded.length > 3 ? (loaded[3] as SnippetPage).snippets : [];
      final present = {for (final row in _merged) row.id};
      _selection.removeWhere((id) => !present.contains(id));
      _status = SnippetsStatus.ready;
      _error = null;
    } on Object catch (error) {
      if (_disposed || generation != _generation) return;
      _status = SnippetsStatus.failed;
      _error = _message(error);
    }
    notifyListeners();
  }

  Future<void> loadMore() async {
    final cursor = _nextCursor;
    if (cursor == null || _loadingMore || searching) return;
    final generation = _generation;
    _loadingMore = true;
    notifyListeners();
    try {
      final page = await _repository.list(_scope, cursor: cursor);
      if (_disposed || generation != _generation) return;
      _rows = [..._rows, ...page.snippets];
      _nextCursor = page.nextCursor;
      _total = page.total;
    } on Object catch (error) {
      if (_disposed || generation != _generation) return;
      _error = _message(error);
    } finally {
      _loadingMore = false;
      notifyListeners();
    }
  }

  void setScope(SnippetListScope scope) {
    if (scope == _scope && !searching && !_attentionOnly) return;
    _scope = scope;
    _query = '';
    _attentionOnly = false;
    _selection.clear();
    _anchor = null;
    _rows = const [];
    _status = SnippetsStatus.loading;
    notifyListeners();
    unawaited(_reload(keepLoaded: false));
  }

  void setQuery(String query) {
    if (query == _query) return;
    _query = query;
    _selection.clear();
    unawaited(_reload(keepLoaded: false));
  }

  void setAttentionOnly(bool value) {
    if (_attentionOnly == value) return;
    _attentionOnly = value;
    notifyListeners();
  }

  /// Toggles [id]; [range] extends from the last toggled row through the
  /// list order (Shift-click).
  void toggleSelection(String id, {bool range = false}) {
    final order = [...pinned, ...rows].map((row) => row.id).toList();
    final anchor = _anchor;
    if (range && anchor != null && order.contains(anchor)) {
      final from = order.indexOf(anchor);
      final to = order.indexOf(id);
      if (to >= 0) {
        final (start, end) = from <= to ? (from, to) : (to, from);
        _selection.addAll(order.sublist(start, end + 1));
      }
    } else if (!_selection.remove(id)) {
      _selection.add(id);
    }
    _anchor = id;
    notifyListeners();
  }

  void clearSelection() {
    if (_selection.isEmpty) return;
    _selection.clear();
    _anchor = null;
    notifyListeners();
  }

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
      if (!_disposed) {
        notifyListeners();
        unawaited(_reload());
      }
    }
  }

  static String _message(Object error) => error is SnippetException
      ? error.message
      : AppStrings.current.snippetsLoadFailed;

  Future<SnippetDetail> detail(String id) => _repository.get(id);

  /// Moves [ids] to Recently Deleted; [undoDelete] restores them.
  Future<bool> delete(List<String> ids) => _mutate(() async {
    await _repository.delete(ids);
    _selection.removeAll(ids);
  });

  Future<bool> undoDelete(List<String> ids) =>
      _mutate(() => _repository.restore(ids));

  Future<bool> restore(List<String> ids) => _mutate(() async {
    await _repository.restore(ids);
    _selection.removeAll(ids);
  });

  Future<bool> purge(List<String> ids) => _mutate(() async {
    await _repository.purge(ids);
    _selection.removeAll(ids);
  });

  Future<bool> move(List<String> ids, String? collectionId) =>
      _mutate(() async {
        await _repository.move(ids, collectionId);
        _selection.removeAll(ids);
      });

  Future<bool> setPinned(String id, bool pinned) =>
      _mutate(() => _repository.setPinned(id, pinned: pinned));

  Future<bool> saveCollection(SnippetCollectionDraft draft) =>
      _mutate(() => _repository.saveCollection(draft));

  /// [deleteSnippets] false moves the collection's snippets to Unfiled.
  Future<bool> deleteCollection(String id, {required bool deleteSnippets}) =>
      _mutate(() async {
        await _repository.deleteCollection(id, deleteSnippets: deleteSnippets);
        if (_scope.collectionId == id) _scope = SnippetListScope.all;
      });

  Future<bool> mergeCollections(String from, String into) => _mutate(() async {
    await _repository.mergeCollections(from, into);
    if (_scope.collectionId == from) {
      _scope = SnippetListScope.collection(into);
    }
  });

  Future<bool> setPreference(SnippetPreference preference, bool value) =>
      _mutate(
        () async =>
            _preferences = await _repository.setPreference(preference, value),
      );

  /// Copies a snippet's output; [values] fill its template fields.
  Future<bool> copy(String id, {Map<String, String> values = const {}}) =>
      _mutate(
        () =>
            _repository.use(id, values: values, delivery: SnippetDelivery.copy),
      );

  /// Whether `snippets.require_auth` is on and the unlock window is closed.
  bool get ownerLocked =>
      _preferences.requireAuth && !(access?.unlocked ?? false);

  /// Whether using or revealing [snippet] needs the device owner first.
  bool needsUnlock(SnippetSummary snippet) => snippet.concealed && ownerLocked;

  @override
  void notifyListeners() {
    if (!_disposed) super.notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    unawaited(_changes?.cancel());
    editorRequest.dispose();
    super.dispose();
  }
}
