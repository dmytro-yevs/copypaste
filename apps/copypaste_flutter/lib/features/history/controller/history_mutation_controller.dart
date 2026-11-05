import '../models/history_models.dart';
import 'history_controller.dart';
import 'history_errors.dart';
import 'history_media_cache.dart';
import 'history_scope.dart';
import 'history_spoiler_controller.dart';

/// Single-clip secret, pin and delete changes, and deleting unpinned history,
/// with their pending state.
class HistoryMutationController {
  HistoryMutationController(
    this._scope,
    this._history, {
    required HistoryErrors errors,
    required HistorySpoilerController spoilers,
    required HistoryMediaCache media,
  }) : _errors = errors,
       _spoilers = spoilers,
       _media = media;

  final HistoryScope _scope;
  final HistoryController _history;
  final HistoryErrors _errors;
  final HistorySpoilerController _spoilers;
  final HistoryMediaCache _media;
  final Set<String> _secretMutations = <String>{};
  final Set<String> _pinMutations = <String>{};
  final Set<String> _deleteMutations = <String>{};
  bool _isDeletingAll = false;

  bool get isDeletingAll => _isDeletingAll;
  bool get hasPending =>
      _secretMutations.isNotEmpty ||
      _pinMutations.isNotEmpty ||
      _deleteMutations.isNotEmpty;
  bool isSecretPending(String id) => _secretMutations.contains(id);
  bool isPinPending(String id) =>
      _history.pinnedOrder.isReorderingPinned || _pinMutations.contains(id);
  bool isDeletePending(String id) =>
      _history.pinnedOrder.isReorderingPinned || _deleteMutations.contains(id);

  bool get _locked =>
      _history.bulk.isBulkMutating || _history.pinnedOrder.isInteracting;

  Future<bool> toggleSecret(HistoryClip clip) async {
    if (_scope.disposed || !_secretMutations.add(clip.id)) return false;
    final selection = _history.selection;
    selection.invalidateDetail(clip.id);
    _scope.notify();
    try {
      final updated = await _scope.repository.setSecret(clip.id, !clip.secret);
      if (_scope.disposed) return true;
      final concealed = _spoilers.setRevealed(clip.id, false);
      _media.forgetSpoilerContent(clip.id);
      _history.window.replace(updated);
      selection.showDetail(clip.id, updated);
      _scope.notify();
      await concealed;
      return true;
    } catch (_) {
      if (!_scope.disposed) {
        _errors.message = 'The clip secrecy could not be updated.';
      }
      return false;
    } finally {
      _secretMutations.remove(clip.id);
      if (!_scope.disposed) _scope.notify();
    }
  }

  Future<bool> togglePin(HistoryClip clip) async {
    if (_locked) return false;
    if (!_pinMutations.add(clip.id)) {
      return false;
    }
    final window = _history.window;
    final next = !clip.pinned;
    if (window.hasEvictedPages) await window.beforeWindowChange?.call();
    if (_scope.disposed) {
      _pinMutations.remove(clip.id);
      window.afterWindowChange?.call();
      return false;
    }
    window.replace(clip.copyWith(pinned: next));
    _history.selection.setPinned(clip.id, next);
    if (window.hasEvictedPages) window.afterWindowChange?.call();
    _scope.notify();
    try {
      await _scope.repository.setPinned(clip.id, next);
      return true;
    } catch (_) {
      window.replace(clip);
      _history.selection.setPinned(clip.id, clip.pinned);
      _errors.reportOperation(
        'The pin could not be updated.',
        action: HistoryAction.pin,
      );
      _scope.notify();
      return false;
    } finally {
      _pinMutations.remove(clip.id);
      if (!_scope.disposed) {
        _scope.notify();
      }
    }
  }

  Future<bool> deleteSelected() async {
    final id = _history.selection.selectedId;
    return id == null ? false : deleteClip(id);
  }

  Future<bool> deleteClip(String id) async {
    if (_locked) return false;
    if (!_deleteMutations.add(id)) {
      return false;
    }
    final window = _history.window;
    _scope.notify();
    try {
      await _scope.repository.delete(id);
      if (window.hasEvictedPages) await window.beforeWindowChange?.call();
      if (_scope.disposed) {
        window.afterWindowChange?.call();
        return true;
      }
      window.removeClip(id);
      _history.bulk.deselect(id);
      if (_history.selection.selectedId == id) {
        _history.selection.clearSelection();
      }
      _media.trimSourceIcons();
      window.updateEmptyState();
      if (window.hasEvictedPages) window.afterWindowChange?.call();
      _scope.notify();
      return true;
    } catch (_) {
      _errors.reportOperation(
        'This clip could not be deleted.',
        action: HistoryAction.delete,
      );
      _scope.notify();
      return false;
    } finally {
      _deleteMutations.remove(id);
      if (!_scope.disposed) {
        _scope.notify();
      }
    }
  }

  Future<bool> deleteAll() async {
    if (_locked || _isDeletingAll) return false;
    final window = _history.window;
    final selection = _history.selection;
    _isDeletingAll = true;
    _scope.notify();
    try {
      await _scope.repository.deleteAll();
      if (window.hasEvictedPages || window.canLoadMore) {
        final selected = selection.selectedClip;
        if (selected != null && !selected.pinned) selection.clearSelection();
        await window.reload();
        return true;
      }
      window.keepPinnedOnly();
      _history.bulk.prune();
      final selectedId = selection.selectedId;
      if (selectedId != null && window.clipById(selectedId) == null) {
        selection.clearSelection();
      }
      _media.trimSourceIcons();
      _scope.notify();
      return true;
    } catch (_) {
      _errors.message = 'History could not be deleted.';
      _scope.notify();
      return false;
    } finally {
      _isDeletingAll = false;
      if (!_scope.disposed) {
        _scope.notify();
      }
    }
  }
}
