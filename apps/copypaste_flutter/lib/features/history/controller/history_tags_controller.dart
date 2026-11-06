import 'dart:async';

import 'package:flutter/foundation.dart';

import '../models/history_models.dart';
import '../repository/history_repository.dart';

/// Owns the bounded tag catalogue, explicit selection, and mutation lifecycle.
class HistoryTagsController extends ChangeNotifier {
  HistoryTagsController(this._repository, {ValueListenable<int>? changes})
    : _changes = changes {
    _changes?.addListener(_changed);
  }
  final ValueListenable<int>? _changes;
  bool _reloadPending = false;
  void _changed() {
    if (_disposed) return;
    if (loading || busy) {
      _reloadPending = true;
      return;
    }
    unawaited(load(_clipIds));
  }

  final HistoryTagsRepository? _repository;
  HistoryTags _tagData = const HistoryTags();
  Map<String, String>? _names;
  HistoryTags get _data => _tagData;
  set _data(HistoryTags value) {
    _tagData = value;
    _names = null;
  }

  bool _disposed = false;
  int _generation = 0;
  bool loading = false;
  bool busy = false;
  String? error;
  List<String> _clipIds = const [];

  bool get available => _repository != null;
  List<HistoryTag> get tags => _data.tags;
  List<String> get clipIds => _clipIds;

  /// Catalogue names of [ids]; tags missing from the catalogue are skipped.
  List<String> names(List<String> ids) {
    if (ids.isEmpty) return const [];
    final names = _names ??= {for (final tag in _data.tags) tag.id: tag.name};
    return [for (final id in ids) ?names[id]];
  }

  int assignedCount(String tagId) => _clipIds
      .where((id) => _data.assignments[id]?.contains(tagId) ?? false)
      .length;

  Future<void> load([List<String> clipIds = const []]) async {
    final repository = _repository;
    if (_disposed || repository == null) return;
    final generation = ++_generation;
    _clipIds = List.unmodifiable(clipIds);
    loading = true;
    error = null;
    notifyListeners();
    try {
      final data = await repository.tags(_clipIds);
      if (_disposed || generation != _generation) return;
      _data = data;
    } catch (_) {
      if (!_disposed && generation == _generation) {
        error = 'Tags could not be loaded. Try again.';
      }
    } finally {
      if (!_disposed && generation == _generation) {
        loading = false;
        notifyListeners();
        if (_reloadPending && !busy) {
          _reloadPending = false;
          unawaited(load(_clipIds));
        }
      }
    }
  }

  Future<bool> _change(
    Future<void> Function(HistoryTagsRepository) action,
  ) async {
    final repository = _repository;
    if (_disposed || busy || repository == null) return false;
    busy = true;
    error = null;
    notifyListeners();
    try {
      await action(repository);
      if (_disposed) return false;
      await load(_clipIds);
      return error == null;
    } catch (_) {
      if (!_disposed) {
        error =
            'The tag change could not be saved. Use a unique name of up to 64 characters and select pinned clips.';
      }
      return false;
    } finally {
      if (!_disposed) {
        busy = false;
        notifyListeners();
        if (_reloadPending) {
          _reloadPending = false;
          unawaited(load(_clipIds));
        }
      }
    }
  }

  Future<bool> save(String name, {String? id}) =>
      _change((repository) => repository.saveTag(id: id, name: name.trim()));
  Future<bool> delete(String id) =>
      _change((repository) => repository.deleteTag(id));
  Future<bool> assign(String tagId, bool assigned) => _change(
    (repository) => repository.changeTags(
      clipIds: _clipIds,
      tagIds: [tagId],
      assigned: assigned,
    ),
  );

  void updateCatalogue(List<HistoryTag> tags) {
    if (_disposed || _clipIds.isNotEmpty || busy || loading) return;
    _data = HistoryTags(tags: tags);
  }

  void releaseSelection() {
    _generation++;
    loading = false;
    _clipIds = const [];
    _data = HistoryTags(tags: _data.tags);
    error = null;
  }

  void clear() {
    releaseSelection();
    _data = const HistoryTags();
  }

  @override
  void dispose() {
    _disposed = true;
    _changes?.removeListener(_changed);
    _generation++;
    _data = const HistoryTags();
    _clipIds = const [];
    super.dispose();
  }
}
