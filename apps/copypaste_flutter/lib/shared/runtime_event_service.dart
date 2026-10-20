import 'dart:async';

import 'package:copypaste_flutter/generated/api.dart' as runtime;

/// One lazy runtime connection shared by features in this engine.
class RuntimeEventService {
  RuntimeEventService({
    Future<BigInt> Function()? allocate,
    Future<void> Function(BigInt)? cancel,
    Stream<runtime.RuntimeEvent> Function(BigInt)? watch,
  }) : _allocate = allocate ?? runtime.allocateRuntimeWatch,
       _cancel =
           cancel ?? ((id) async => runtime.cancelRuntimeWatch(watchId: id)),
       _watch = watch ?? ((id) => runtime.watchRuntime(watchId: id)) {
    _events = StreamController<runtime.RuntimeEvent>.broadcast(
      onListen: _start,
      onCancel: _stop,
    );
  }

  static final instance = RuntimeEventService();
  final Future<BigInt> Function() _allocate;
  final Future<void> Function(BigInt) _cancel;
  final Stream<runtime.RuntimeEvent> Function(BigInt) _watch;
  late final StreamController<runtime.RuntimeEvent> _events;
  StreamSubscription<runtime.RuntimeEvent>? _subscription;
  Completer<bool>? _ready;
  BigInt? _id;
  Timer? _retry;
  int _generation = 0;

  Stream<runtime.RuntimeEvent> get events => _events.stream;

  /// Must be called while listening to [events]. Completes after the backend
  /// has subscribed, so a file operation cannot race its progress watcher.
  Future<bool> get ready => _ready!.future;

  void _start() {
    final generation = ++_generation;
    final ready = _ready != null && !_ready!.isCompleted
        ? _ready!
        : (_ready = Completer<bool>());
    ready.future.ignore();
    var reportedError = false;
    () async {
      try {
        final id = await _allocate();
        if (generation != _generation) {
          await _cancel(id);
          return;
        }
        _id = id;
        _subscription = _watch(id).listen(
          (event) {
            if (generation != _generation) return;
            if (event.kind == 'ready' && !ready.isCompleted) {
              ready.complete(event.runtimeWatch);
            }
            _events.add(event);
          },
          onError: (Object error, StackTrace trace) {
            if (generation != _generation) return;
            if (!ready.isCompleted) ready.completeError(error, trace);
            if (!reportedError) _ready = Completer<bool>()..future.ignore();
            reportedError = true;
            _events.addError(error, trace);
          },
          onDone: () {
            if (!ready.isCompleted) {
              ready.completeError(StateError('Runtime watch closed'));
            }
            if (generation != _generation) return;
            if (!reportedError) {
              _events.addError(StateError('Runtime watch disconnected'));
            }
            _id = null;
            _subscription = null;
            if (!reportedError) _ready = Completer<bool>()..future.ignore();
            _cancel(id);
            _retry = Timer(const Duration(seconds: 1), () {
              if (generation == _generation && _events.hasListener) {
                _start();
              }
            });
          },
        );
      } catch (error, trace) {
        if (generation != _generation) return;
        final id = _id;
        _id = null;
        if (id != null) {
          try {
            await _cancel(id);
          } catch (_) {
            // Report the original startup failure below.
          }
        }
        if (!ready.isCompleted) ready.completeError(error, trace);
        _events.addError(error, trace);
        if (generation == _generation && _events.hasListener) {
          _ready = Completer<bool>()..future.ignore();
          _retry = Timer(const Duration(seconds: 1), () {
            if (generation == _generation && _events.hasListener) _start();
          });
        }
      }
    }();
  }

  void _stop() {
    ++_generation;
    _retry?.cancel();
    _retry = null;
    final subscription = _subscription;
    final id = _id;
    _subscription = null;
    _id = null;
    _ready = null;
    subscription?.cancel();
    if (id != null) _cancel(id);
  }
}
