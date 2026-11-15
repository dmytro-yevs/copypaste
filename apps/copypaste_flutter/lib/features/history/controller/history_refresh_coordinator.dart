import 'dart:async';

import 'history_scope.dart';

/// Runs one visible-row refresh at a time and holds one deferred refresh while
/// an interaction owns the rows, replaying it when the interaction ends.
class HistoryRefreshCoordinator {
  HistoryRefreshCoordinator(
    this._scope, {
    required Future<void> Function() refresh,
    required void Function() invalidate,
    required bool Function() held,
    required bool Function() interrupted,
  }) : _refresh = refresh,
       _invalidate = invalidate,
       _held = held,
       _interrupted = interrupted;

  final HistoryScope _scope;
  final Future<void> Function() _refresh;
  final void Function() _invalidate;

  /// An interaction that defers new refreshes until it ends.
  final bool Function() _held;

  /// An interaction that cancels a running refresh and takes its queued pass.
  final bool Function() _interrupted;

  Future<void>? _running;
  bool _queued = false;
  int _generation = 0;
  bool _deferred = false;

  Future<void>? get inFlight => _running;

  /// Requests made while a refresh runs coalesce into one follow-up pass; the
  /// returned future completes after that pass.
  Future<void> run() {
    final running = _running;
    if (running != null) {
      _queued = true;
      return running;
    }
    final generation = _generation;
    late final Future<void> refresh;
    refresh = () async {
      try {
        while (true) {
          await _refresh();
          if (!_queued || generation != _generation) return;
          _queued = false;
          if (_scope.disposed || _scope.suspended) return;
          if (_interrupted()) {
            _deferred = true;
            return;
          }
        }
      } finally {
        // Cleared with the final queue check so no request joins a done pass.
        if (identical(_running, refresh)) _running = null;
      }
    }();
    _running = refresh;
    return refresh;
  }

  /// Defers the refresh when an interaction holds refreshes.
  bool hold() {
    if (!_held()) return false;
    _deferred = true;
    return true;
  }

  void defer() => _deferred = true;

  void discard() => _deferred = false;

  /// Cancels a running refresh and replays it after the interaction.
  void interrupt() {
    if (_running == null) return;
    _invalidate();
    _deferred = true;
  }

  /// Replays a deferred refresh once no interaction holds refreshes.
  Future<void> flush() {
    if (_scope.disposed || !_deferred || _held()) return Future.value();
    _deferred = false;
    return run();
  }

  /// Refreshes after an interaction that held refreshes while it ran, and
  /// once more when a change arrived during that refresh.
  Future<void> settle() async {
    _deferred = false;
    await run();
    if (_deferred) {
      _deferred = false;
      await run();
    }
  }

  /// A first page supersedes the queued row pass.
  bool takeQueued() {
    final queued = _queued;
    _queued = false;
    return queued;
  }

  void reset() {
    _running = null;
    _queued = false;
    _generation++;
  }
}
