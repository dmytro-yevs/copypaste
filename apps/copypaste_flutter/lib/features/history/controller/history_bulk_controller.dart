import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/foundation.dart';

import 'history_controller.dart';
import 'history_errors.dart';
import 'history_media_cache.dart';
import 'history_refresh_coordinator.dart';
import 'history_scope.dart';
import 'history_window_controller.dart';

enum _HistoryBulkMutation { delete, pin, unpin }

/// Multi-clip selection, drag selection and the bulk delete and pin actions.
class HistoryBulkController {
  HistoryBulkController(
    this._scope,
    this._history, {
    required HistoryErrors errors,
    required HistoryMediaCache media,
    required HistoryRefreshCoordinator refresh,
  }) : _errors = errors,
       _media = media,
       _refresh = refresh;

  final HistoryScope _scope;
  final HistoryController _history;
  final HistoryErrors _errors;
  final HistoryMediaCache _media;
  final HistoryRefreshCoordinator _refresh;
  final Set<String> _bulkSelectedIds = <String>{};
  String? _bulkAnchorId;
  Set<String>? _bulkDragBaseline;
  bool _isBulkSelecting = false;
  bool _isBulkMutating = false;

  bool get isBulkSelecting => _isBulkSelecting;
  bool get isBulkMutating => _isBulkMutating;
  bool get isBulkDragSelecting => _bulkDragBaseline != null;
  Set<String> get bulkSelectedIds => Set.unmodifiable(_bulkSelectedIds);
  bool isBulkSelected(String id) => _bulkSelectedIds.contains(id);

  bool get canBeginBulkSelection =>
      !_scope.disposed &&
      !_scope.suspended &&
      _history.window.state == HistoryLoadState.ready &&
      !_history.transfers.isImportingFiles &&
      !_history.mutations.isDeletingAll &&
      !_history.pinnedOrder.isReorderingPinned &&
      !_isBulkMutating &&
      _history.pinnedOrder.draggedPinnedId == null &&
      !_history.mutations.hasPending;

  bool get canTagBulkSelection {
    if (!_history.tags.available ||
        _bulkSelectedIds.isEmpty ||
        _bulkSelectedIds.length > 500) {
      return false;
    }
    final pinned = _history.window.ledgerPinnedIds.toSet();
    return _bulkSelectedIds.every(pinned.contains);
  }

  void beginBulkSelection([String? id]) {
    if (!canBeginBulkSelection) return;
    if (id != null && _history.window.clipById(id) == null) return;
    _history.window.cancelSearch();
    _isBulkSelecting = true;
    _history.selection.release();
    if (id != null) {
      _bulkSelectedIds.add(id);
      _bulkAnchorId = id;
    }
    _scope.notify();
  }

  void toggleBulkSelection(String id) {
    if (!_isBulkSelecting ||
        _isBulkMutating ||
        _history.window.clipById(id) == null) {
      return;
    }
    if (!_bulkSelectedIds.add(id)) _bulkSelectedIds.remove(id);
    _bulkAnchorId = id;
    _scope.notify();
  }

  /// Range order comes from the rendered list, including its active section
  /// expansion and sort order, rather than the repository's page order.
  void selectBulkClip(
    String id, {
    required List<String> orderedIds,
    bool range = false,
    bool additive = false,
  }) {
    if (_scope.disposed || _isBulkMutating || !orderedIds.contains(id)) return;
    final anchor = _bulkAnchorId ?? _history.selection.selectedId ?? id;
    final previousSingle = _history.selection.selectedId;
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
    final visited = _history.window.visibleVisitedIds;
    final order = orderedIds.contains(anchor) && orderedIds.contains(id)
        ? orderedIds
        : visited;
    final ids = _bulkRange(anchor, id, order);
    if (!additive) _bulkSelectedIds.clear();
    _bulkSelectedIds.addAll(ids);
    _bulkAnchorId = anchor;
    _scope.notify();
  }

  List<String> _bulkRange(String anchor, String id, List<String> orderedIds) {
    var start = orderedIds.indexOf(anchor);
    final end = orderedIds.indexOf(id);
    if (end < 0) return const [];
    if (start < 0) start = end;
    final known = _history.window.ledgerIds.toSet();
    return orderedIds
        .sublist(start < end ? start : end, (start > end ? start : end) + 1)
        .where(known.contains)
        .toList(growable: false);
  }

  bool beginBulkDragSelection(String id) {
    if (!canBeginBulkSelection || _history.window.clipById(id) == null) {
      return false;
    }
    _refresh.interrupt();
    final baseline = Set<String>.of(_bulkSelectedIds);
    beginBulkSelection(id);
    if (!_isBulkSelecting || _bulkAnchorId != id) return false;
    _bulkDragBaseline = baseline;
    _scope.notify();
    return true;
  }

  void updateBulkDragSelection(String id, {required List<String> orderedIds}) {
    final baseline = _bulkDragBaseline;
    final anchor = _bulkAnchorId;
    if (_scope.disposed ||
        baseline == null ||
        anchor == null ||
        _isBulkMutating) {
      return;
    }
    final order = orderedIds.contains(anchor) && orderedIds.contains(id)
        ? orderedIds
        : _history.window.visibleVisitedIds;
    final ids = {...baseline, ..._bulkRange(anchor, id, order)};
    if (setEquals(ids, _bulkSelectedIds)) return;
    _bulkSelectedIds
      ..clear()
      ..addAll(ids);
    _scope.notify();
  }

  void endBulkDragSelection() {
    if (_scope.disposed || _bulkDragBaseline == null) return;
    _bulkDragBaseline = null;
    _scope.notify();
    unawaited(_refresh.flush());
  }

  void endBulkSelection() {
    if (_scope.disposed || _isBulkMutating) return;
    endBulkDragSelection();
    _isBulkSelecting = false;
    _bulkSelectedIds.clear();
    _bulkAnchorId = null;
    _scope.notify();
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
    _errors.message = null;
    _scope.notify();
    var failed = 0;
    try {
      // Finish an existing live refresh before taking the operation snapshot.
      while (_refresh.inFlight != null) {
        await _refresh.inFlight;
      }
      final ids = List<String>.of(_bulkSelectedIds);
      for (final id in ids) {
        if (_scope.disposed) return false;
        try {
          if (action == _HistoryBulkMutation.delete) {
            await _scope.repository.delete(id);
            if (_scope.disposed) return false;
            _history.window.removeClip(id);
            _bulkSelectedIds.remove(id);
          } else {
            final pinned = action == _HistoryBulkMutation.pin;
            final clip =
                _history.window.clipById(id) ?? await _scope.repository.get(id);
            if (clip.pinned == pinned) continue;
            await _scope.repository.setPinned(clip.id, pinned);
            if (_scope.disposed) return false;
            _history.window.replace(clip.copyWith(pinned: pinned));
          }
        } catch (_) {
          failed++;
        }
      }
      _history.window.updateEmptyState();
      _media.trimSourceIcons();
      if (action == _HistoryBulkMutation.delete && _bulkSelectedIds.isEmpty) {
        _isBulkSelecting = false;
      }
    } finally {
      if (!_scope.disposed) {
        // Mutations can change both the pin order and pagination cursors.
        try {
          await _refresh.settle();
        } finally {
          _isBulkMutating = false;
          if (failed > 0) {
            _errors.message = action == _HistoryBulkMutation.delete
                ? AppStrings.current.historyErrorBulkDelete
                : AppStrings.current.historyErrorBulkPin;
          }
          if (!_scope.disposed) _scope.notify();
        }
      }
    }
    return failed == 0;
  }

  /// Drops selected IDs that left the loaded ledger.
  void prune() {
    final ids = _history.window.ledgerIds.toSet();
    _bulkSelectedIds.removeWhere((id) => !ids.contains(id));
    if (!ids.contains(_bulkAnchorId)) _bulkAnchorId = null;
  }

  void deselect(String id) => _bulkSelectedIds.remove(id);

  void suspend() {
    _isBulkSelecting = false;
    _bulkSelectedIds.clear();
    _bulkAnchorId = null;
    _bulkDragBaseline = null;
  }
}
