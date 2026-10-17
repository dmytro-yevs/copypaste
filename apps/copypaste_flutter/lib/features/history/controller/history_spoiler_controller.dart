import 'package:flutter/foundation.dart';

class HistorySpoilerState {
  const HistorySpoilerState(this.revision, this.revealedIds);
  final int revision;
  final Set<String> revealedIds;
}

abstract interface class HistorySpoilerRepository {
  Future<HistorySpoilerState> readSpoilers();
  Future<HistorySpoilerState> setSpoilerRevealed(String? id, bool revealed);
}

/// One revisioned presentation state for every clip surface in this engine.
/// The runtime repository shares the same state with other local engines.
class HistorySpoilerController extends ChangeNotifier {
  HistorySpoilerController(this._repository);

  final HistorySpoilerRepository? _repository;
  HistorySpoilerState _state = const HistorySpoilerState(-1, {});
  bool _disposed = false;
  int _generation = 0;

  bool isRevealed(String id) => _state.revealedIds.contains(id);

  void _apply(HistorySpoilerState state) {
    if (_disposed || state.revision < _state.revision) return;
    final changed = !setEquals(state.revealedIds, _state.revealedIds);
    _state = state;
    if (changed) notifyListeners();
  }

  void disconnected() {
    if (_disposed) return;
    _generation++;
    final revealed = _state.revealedIds.isNotEmpty;
    _state = const HistorySpoilerState(-1, {});
    if (revealed) notifyListeners();
  }

  Future<void> refresh() async {
    final repository = _repository;
    if (_disposed || repository == null) return;
    final generation = _generation;
    try {
      final state = await repository.readSpoilers();
      if (generation == _generation) _apply(state);
    } catch (_) {
      if (!_disposed && generation == _generation) {
        _apply(HistorySpoilerState(_state.revision, {}));
      }
    }
  }

  Future<bool> setRevealed(String? id, bool revealed) async {
    if (_disposed) return false;
    final generation = ++_generation;
    // Concealing takes effect before the backend round trip; revealing waits
    // for its acknowledgement so a failed request never exposes content.
    if (!revealed) {
      _apply(
        HistorySpoilerState(_state.revision, {
          if (id != null) ..._state.revealedIds.where((value) => value != id),
        }),
      );
    }
    try {
      final repository = _repository;
      final state = repository == null
          ? HistorySpoilerState(_state.revision + 1, {
              ..._state.revealedIds,
              if (revealed && id != null) id,
            })
          : await repository.setSpoilerRevealed(id, revealed);
      if (generation == _generation) _apply(state);
      return true;
    } catch (_) {
      return false;
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
