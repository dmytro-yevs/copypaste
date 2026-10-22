import 'dart:async';

import 'package:flutter/foundation.dart';

enum FileOperationKind { import, receive, send }

class FileOperation {
  const FileOperation({
    required this.id,
    required this.itemId,
    required this.name,
    required this.kind,
    required this.phase,
    required this.sourceDevice,
    required this.destinationDevice,
    required this.written,
    required this.total,
    required this.startedAt,
    this.finishedAt,
    this.error,
    this.localPath,
  });
  final String id, itemId, name, phase, sourceDevice, destinationDevice;
  final FileOperationKind kind;
  final int written, total;
  final DateTime startedAt;
  final DateTime? finishedAt;
  final String? error, localPath;
  bool get active => phase == 'running';
  bool get failed => phase == 'failed' || phase == 'interrupted';
  double? get progress => total > 0 ? (written / total).clamp(0, 1) : null;
  String get label => switch (phase) {
    'running' => switch (kind) {
      FileOperationKind.send => 'Sending',
      FileOperationKind.receive => 'Downloading',
      FileOperationKind.import => 'Adding',
    },
    'completed' => switch (kind) {
      FileOperationKind.import => 'Added',
      FileOperationKind.send => 'Sent',
      FileOperationKind.receive => 'Completed',
    },
    'cancelled' => 'Cancelled',
    'interrupted' => 'Interrupted',
    _ => 'Failed',
  };
  FileOperation withProgress(int bytes, int size) => FileOperation(
    id: id,
    itemId: itemId,
    name: name,
    kind: kind,
    phase: phase,
    sourceDevice: sourceDevice,
    destinationDevice: destinationDevice,
    written: bytes,
    total: size,
    startedAt: startedAt,
    finishedAt: finishedAt,
    error: error,
    localPath: localPath,
  );
}

class FileOperationsChange {
  const FileOperationsChange({
    this.id,
    this.written = 0,
    this.total = 0,
    this.finished = true,
  });
  final String? id;
  final int written, total;
  final bool finished;
}

abstract interface class FileOperationsRepository {
  Stream<FileOperationsChange> watch();
  Future<List<FileOperation>> load({
    bool clear = false,
    String? before,
    String? id,
  });
  Future<void> retry(FileOperation operation);
  Future<void> cancel(String id);
  Future<void> openFile(String path);
}

/// Presentation lifetime never owns or cancels a runtime transfer worker.
class FileOperationsController extends ChangeNotifier {
  FileOperationsController(this._repository, {Duration Function()? now})
    : _now = now ?? _monotonicNow;

  static final Stopwatch _clock = Stopwatch()..start();
  static Duration _monotonicNow() => _clock.elapsed;

  final FileOperationsRepository _repository;
  final Duration Function() _now;
  StreamSubscription<FileOperationsChange>? _watch;
  List<FileOperation> _items = const [];
  List<FileOperation> get items => _items;
  Iterable<FileOperation> get active => _items.where((item) => item.active);
  String? error;
  bool _disposed = false, _refreshAgain = false;
  Future<void>? _refresh;
  String? _before;
  int _generation = 0;
  bool hasEarlier = false;
  bool get showingEarlier => _before != null;
  final Set<String> _loadingIds = {};
  final Set<String> _reloadIds = {};
  final Map<String, FileOperation> _clipStates = {};
  final Map<String, _TransferSpeedSample> _speedSamples = {};
  static const _speedStaleAfter = Duration(seconds: 5);

  /// Returns a short-window, event-driven speed estimate for an active transfer.
  double? speedFor(String id) {
    final sample = _speedSamples[id];
    if (sample == null || _now() - sample.sampledAt > _speedStaleAfter) {
      return null;
    }
    return sample.bytesPerSecond;
  }

  void _recordSpeed(FileOperationsChange change) {
    final id = change.id;
    if (id == null) return;
    final now = _now();
    final previous = _speedSamples[id];
    if (previous == null) {
      _speedSamples[id] = _TransferSpeedSample(
        written: change.written,
        sampledAt: now,
      );
      return;
    }
    final elapsed = now - previous.sampledAt;
    final written = change.written - previous.written;
    if (elapsed <= Duration.zero || written <= 0) return;
    final instant =
        written * Duration.microsecondsPerSecond / elapsed.inMicroseconds;
    _speedSamples[id] = _TransferSpeedSample(
      written: change.written,
      sampledAt: now,
      bytesPerSecond: previous.bytesPerSecond == null
          ? instant
          : previous.bytesPerSecond! * 0.6 + instant * 0.4,
    );
  }

  void _retainActiveSpeedSamples(Iterable<FileOperation> records) {
    final activeIds = records
        .where((record) => record.active)
        .map((record) => record.id)
        .toSet();
    _speedSamples.removeWhere((id, _) => !activeIds.contains(id));
  }

  void _remember(Iterable<FileOperation> records) {
    for (final record in records) {
      if (record.kind != FileOperationKind.receive) {
        continue;
      }
      final previous = _clipStates[record.itemId];
      if (previous != null) {
        final order = previous.startedAt.compareTo(record.startedAt);
        if (order > 0 || (order == 0 && previous.id.compareTo(record.id) > 0)) {
          continue;
        }
      }
      // Saved-byte reuse belongs to the runtime's downloaded-file mapping;
      // an older saved path must never replace a newer operation's status.
      _clipStates.remove(record.itemId);
      _clipStates[record.itemId] = record;
    }
    while (_clipStates.length > 64) {
      final removable = _clipStates.entries
          .where((entry) => !entry.value.active)
          .firstOrNull;
      if (removable == null) break;
      _clipStates.remove(removable.key);
    }
  }

  FileOperation? forClip(String id) {
    final running = _items
        .where(
          (item) =>
              item.itemId == id &&
              item.kind == FileOperationKind.receive &&
              item.active,
        )
        .toList(growable: false);
    if (running.isNotEmpty) {
      return running.reduce(
        (first, next) =>
            first.startedAt.isBefore(next.startedAt) ? first : next,
      );
    }
    return _clipStates[id];
  }

  Future<void> initialize() async {
    _watch ??= _repository.watch().listen(
      (change) {
        final index = _items.indexWhere((item) => item.id == change.id);
        if (!change.finished && index >= 0) {
          _recordSpeed(change);
          _items = List.unmodifiable([
            for (var i = 0; i < _items.length; i++)
              i == index
                  ? _items[i].withProgress(change.written, change.total)
                  : _items[i],
          ]);
          _remember([_items[index]]);
          notifyListeners();
        } else if (change.id case final id?) {
          _speedSamples.remove(id);
          unawaited(_loadChanged(id, terminal: change.finished));
        } else {
          _generation++;
          _before = null;
          _clipStates.clear();
          _speedSamples.clear();
          unawaited(refresh());
        }
      },
      onError: (Object _) {
        if (_disposed) return;
        error = 'Transfer updates are unavailable. Reopen to refresh.';
        notifyListeners();
      },
    );
    await refresh();
  }

  Future<void> refresh() {
    if (_disposed) return Future.value();
    if (_refresh case final pending?) {
      _refreshAgain = true;
      return pending;
    }
    return _refresh = _load();
  }

  Future<void> _load() async {
    try {
      do {
        _refreshAgain = false;
        final generation = _generation;
        final records = await _repository.load(before: _before);
        if (generation != _generation) {
          _refreshAgain = true;
          continue;
        }
        hasEarlier = records.where((item) => !item.active).length == 50;
        if (_disposed) return;
        final absentActive = _clipStates.values
            .where(
              (item) =>
                  item.active && !records.any((record) => record.id == item.id),
            )
            .toList(growable: false);
        for (final previous in absentActive) {
          final result = await _repository.load(id: previous.id);
          if (_disposed) return;
          _remember(result);
        }
        _remember(records);
        _retainActiveSpeedSamples(records);
        _items = List.unmodifiable([
          ...records.where((item) => item.active),
          ...records.where((item) => !item.active),
        ]);
        error = null;
        notifyListeners();
      } while (_refreshAgain);
    } catch (_) {
      if (!_disposed) {
        error = 'Transfers could not be loaded. Try again.';
        notifyListeners();
      }
    } finally {
      _refresh = null;
    }
  }

  Future<void> _loadChanged(String id, {required bool terminal}) async {
    if (_disposed) return;
    if (_loadingIds.length >= 4 && !_loadingIds.contains(id)) {
      unawaited(refresh());
      return;
    }
    if (!_loadingIds.add(id)) {
      if (terminal) _reloadIds.add(id);
      return;
    }
    final generation = _generation;
    try {
      do {
        _reloadIds.remove(id);
        final records = await _repository.load(id: id);
        if (_disposed || generation != _generation) return;
        _remember(records);
        final next = {for (final item in _items) item.id: item};
        for (final item in records) {
          if (item.active || _before == null || next.containsKey(item.id)) {
            next[item.id] = item;
          } else {
            next.remove(item.id);
          }
        }
        final sorted = next.values.toList()
          ..sort((a, b) {
            final stamp = b.startedAt.compareTo(a.startedAt);
            return stamp == 0 ? b.id.compareTo(a.id) : stamp;
          });
        _items = List.unmodifiable([
          ...sorted.where((item) => item.active),
          ...sorted.where((item) => !item.active).take(50),
        ]);
        _retainActiveSpeedSamples(_items);
        error = null;
        notifyListeners();
      } while (_reloadIds.contains(id));
    } catch (_) {
      if (!_disposed) {
        error = 'Transfer updates could not be loaded.';
        notifyListeners();
      }
    } finally {
      _loadingIds.remove(id);
      _reloadIds.remove(id);
    }
  }

  Future<void> loadEarlier() async {
    _generation++;
    _before = _items.where((item) => !item.active).lastOrNull?.id;
    await refresh();
  }

  Future<void> showLatest() async {
    _generation++;
    _before = null;
    await refresh();
  }

  Future<void> retry(FileOperation item) async {
    _speedSamples.remove(item.id);
    try {
      await _repository.retry(item);
    } catch (_) {
      if (!_disposed) {
        error = 'This file could not be retried.';
        notifyListeners();
      }
    }
  }

  Future<void> clearHistory() async {
    _generation++;
    _before = null;
    _clipStates.clear();
    try {
      await _repository.load(clear: true);
      await refresh();
    } catch (_) {
      if (!_disposed) {
        error = 'Transfer history could not be cleared.';
        notifyListeners();
      }
    }
  }

  Future<void> removeHistory(FileOperation item) async {
    if (item.active) return;
    _generation++;
    try {
      await _repository.load(clear: true, id: item.id);
      if (_clipStates[item.itemId]?.id == item.id) {
        _clipStates.remove(item.itemId);
      }
      _speedSamples.remove(item.id);
      await refresh();
    } catch (_) {
      if (!_disposed) {
        error = 'This history record could not be removed.';
        notifyListeners();
      }
    }
  }

  Future<void> cancel(FileOperation item) async {
    _speedSamples.remove(item.id);
    try {
      await _repository.cancel(item.id);
    } catch (_) {
      if (!_disposed) {
        error = 'The transfer could not be cancelled. It may still be running.';
        notifyListeners();
      }
    }
  }

  Future<void> openFile(FileOperation item) async {
    final path = item.localPath;
    if (path == null) return;
    try {
      await _repository.openFile(path);
    } catch (_) {
      if (!_disposed) {
        error =
            'The saved file is unavailable. It may have been moved or deleted.';
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    unawaited(_watch?.cancel());
    super.dispose();
  }
}

class _TransferSpeedSample {
  const _TransferSpeedSample({
    required this.written,
    required this.sampledAt,
    this.bytesPerSecond,
  });

  final int written;
  final Duration sampledAt;
  final double? bytesPerSecond;
}
