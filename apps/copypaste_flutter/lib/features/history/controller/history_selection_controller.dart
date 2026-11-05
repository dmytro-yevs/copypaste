import 'dart:async';

import '../models/history_models.dart';
import 'history_controller.dart';
import 'history_errors.dart';
import 'history_scope.dart';
import 'history_spoiler_controller.dart';
import 'history_summary.dart';

/// The single selected clip and the full detail loaded for it.
class HistorySelectionController {
  HistorySelectionController(
    this._scope,
    this._history, {
    required HistoryErrors errors,
    required HistorySpoilerController spoilers,
  }) : _errors = errors,
       _spoilers = spoilers;

  final HistoryScope _scope;
  final HistoryController _history;
  final HistoryErrors _errors;
  final HistorySpoilerController _spoilers;
  String? _selectedId;
  HistoryClip? _selectedClip;
  int _selectedDetailEpoch = 0;
  int? _loadedDetailEpoch;
  Future<void>? _selectedDetailLoad;
  int? _selectedDetailLoadEpoch;

  String? get selectedId => _selectedId;
  HistoryClip? get selectedClip => _selectedClip;

  Future<void> select(String id) async {
    if (_history.bulk.isBulkSelecting) return;
    final queryEpoch = _history.window.queryEpoch;
    final detailEpoch = ++_selectedDetailEpoch;
    _selectedId = id;
    _selectedClip = _history.window.clipById(id);
    _scope.notify();
    await _startSelectedDetailLoad(id, queryEpoch, detailEpoch);
  }

  void clearSelection() {
    _selectedDetailEpoch++;
    _selectedDetailLoad = null;
    _selectedDetailLoadEpoch = null;
    _selectedId = null;
    _selectedClip = null;
    _scope.notify();
  }

  /// Drops the selection without a notification; bulk selection replaces it.
  void release() {
    _selectedId = null;
    _selectedClip = null;
  }

  /// Keeps the loaded detail while its summary row is unchanged; any summary
  /// change, missing row or unfinished detail selects the clip again.
  void refresh() {
    final id = _selectedId;
    if (id == null) return;
    final selected = _selectedClip;
    final row = _history.window.clipById(id);
    if (selected != null &&
        row != null &&
        _loadedDetailEpoch == _selectedDetailEpoch &&
        historySummaryEquals(historySummary(selected), row)) {
      return;
    }
    unawaited(select(id));
  }

  /// Loads the selected detail again after a change event.
  void reloadDetail() {
    final id = _selectedId;
    if (id == null) return;
    final queryEpoch = _history.window.queryEpoch;
    final detailEpoch = ++_selectedDetailEpoch;
    _selectedClip = null;
    _selectedDetailLoad = null;
    _selectedDetailLoadEpoch = null;
    _scope.notify();
    unawaited(_startSelectedDetailLoad(id, queryEpoch, detailEpoch));
  }

  /// Discards an in-flight detail load for [id] before a local update.
  void invalidateDetail(String id) {
    if (_selectedId != id) return;
    _selectedDetailEpoch++;
    _selectedDetailLoad = null;
    _selectedDetailLoadEpoch = null;
  }

  void showDetail(String id, HistoryClip clip) {
    if (_selectedId != id) return;
    invalidateDetail(id);
    _selectedClip = clip;
  }

  void setPinned(String id, bool pinned) {
    if (_selectedId == id) {
      _selectedClip = _selectedClip?.copyWith(pinned: pinned);
    }
  }

  Future<void> awaitCurrentDetail() async {
    final load = _selectedDetailLoad;
    final detailEpoch = _selectedDetailLoadEpoch;
    if (load != null && detailEpoch == _selectedDetailEpoch) await load;
  }

  /// Releases the detail payload while keeping the selection identity.
  void suspend() {
    _selectedClip = null;
    _selectedDetailEpoch++;
    _selectedDetailLoad = null;
    _selectedDetailLoadEpoch = null;
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

  Future<void> _loadSelectedDetail(
    String id,
    int queryEpoch,
    int detailEpoch,
  ) async {
    try {
      final fullClip = await _scope.repository.get(id);
      if (fullClip.secret && _scope.repository is HistorySpoilerRepository) {
        await _spoilers.refresh();
      }
      if (!_history.window.isCurrent(queryEpoch) ||
          _selectedId != id ||
          _selectedDetailEpoch != detailEpoch) {
        return;
      }
      _selectedClip = fullClip;
      _loadedDetailEpoch = detailEpoch;
      _history.window.replace(fullClip);
      _scope.notify();
    } catch (_) {
      if (!_history.window.isCurrent(queryEpoch) ||
          _selectedId != id ||
          _selectedDetailEpoch != detailEpoch) {
        return;
      }
      _errors.message = 'This clip could not be opened.';
      _scope.notify();
    }
  }
}
