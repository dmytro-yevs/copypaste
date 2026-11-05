import 'dart:async';

import 'package:flutter/foundation.dart';

import 'history_controller.dart';
import 'history_errors.dart';
import 'history_refresh_coordinator.dart';
import 'history_scope.dart';
import 'history_window_controller.dart';

/// Dragging and saving the order of loaded pinned clips.
class HistoryPinnedOrderController {
  HistoryPinnedOrderController(
    this._scope,
    this._history, {
    required HistoryErrors errors,
    required HistoryRefreshCoordinator refresh,
  }) : _errors = errors,
       _refresh = refresh;

  final HistoryScope _scope;
  final HistoryController _history;
  final HistoryErrors _errors;
  final HistoryRefreshCoordinator _refresh;
  bool _isReorderingPinned = false;
  String? _draggedPinnedId;
  String? _cancelledPinnedId;

  bool get isReorderingPinned => _isReorderingPinned;
  String? get draggedPinnedId => _draggedPinnedId;
  bool get isInteracting => _draggedPinnedId != null || _isReorderingPinned;

  bool get canReorderPinned {
    final window = _history.window;
    return !_scope.disposed &&
        !_history.bulk.isBulkSelecting &&
        window.hasUnfilteredQuery &&
        window.state == HistoryLoadState.ready &&
        !_isReorderingPinned &&
        !_history.mutations.isDeletingAll &&
        !_history.mutations.hasPending &&
        window.skippedUndecryptable == 0 &&
        window.rows.where((item) => item.pinned).take(2).length == 2;
  }

  void beginPinnedDrag(String id) {
    if (!canReorderPinned ||
        !_history.window.rows.any((item) => item.id == id && item.pinned)) {
      return;
    }
    _refresh.interrupt();
    _draggedPinnedId = id;
    _cancelledPinnedId = null;
    _scope.notify();
  }

  void endPinnedDrag() {
    if (_draggedPinnedId == null) return;
    _draggedPinnedId = null;
    if (!_scope.disposed) _scope.notify();
    // Sortable calls drag-end before its accept callback starts the save.
    scheduleMicrotask(_flushRefresh);
  }

  void cancelPinnedDrag() {
    _cancelledPinnedId = _draggedPinnedId;
    endPinnedDrag();
    // Flutter may end an accepted pan on pointer cancellation. Reject that
    // gesture's drop before clearing the cancellation at the event boundary.
    scheduleMicrotask(() => _cancelledPinnedId = null);
  }

  void _flushRefresh() => unawaited(_refresh.flush());

  Future<bool> shiftPinned(String id, {required bool up}) async {
    final pins = _history.window.rows.where((item) => item.pinned).toList();
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
    final window = _history.window;
    final previous = window.ledgerPinnedIds.toList();
    if (!previous.contains(id) || !previous.contains(targetId)) return false;
    final ordered = previous.where((value) => value != id).toList();
    ordered.insert(ordered.indexOf(targetId) + (before ? 0 : 1), id);
    if (listEquals(previous, ordered)) return false;

    final previousCursor = window.paginationMarker;
    final epoch = window.restartPass();
    _isReorderingPinned = true;
    _errors.message = null;
    if (window.hasEvictedPages) await window.beforeWindowChange?.call();
    if (!window.isCurrent(epoch)) {
      _isReorderingPinned = false;
      window.afterWindowChange?.call();
      return false;
    }
    window.applyPinnedOrder(ordered);
    if (window.hasEvictedPages) window.afterWindowChange?.call();
    _scope.notify();
    try {
      await _scope.repository.reorderPinned(ordered);
    } catch (_) {
      if (window.isCurrent(epoch)) {
        window.restorePinnedOrder(previous, previousCursor);
        _errors.message = 'The pinned order could not be saved. Try again.';
      }
      _isReorderingPinned = false;
      if (!_scope.disposed) _scope.notify();
      _flushRefresh();
      return false;
    }

    try {
      if (window.isCurrent(epoch)) {
        _refresh.discard();
        // Rebuild the continuation marker without clearing the visible list or
        // using a cursor whose last pin may have moved to a different position.
        await window.readVisibleItems(epoch);
      }
    } catch (_) {
      if (window.isCurrent(epoch)) {
        _errors.message =
            'The order was saved, but history could not be refreshed.';
      }
    } finally {
      _isReorderingPinned = false;
      if (!_scope.disposed) _scope.notify();
      _flushRefresh();
    }
    return true;
  }
}
