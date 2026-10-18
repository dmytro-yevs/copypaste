import 'dart:async';
import 'dart:io';

import 'package:path_provider/path_provider.dart';

import '../models/history_models.dart';
import 'history_repository.dart';

class HistoryFileTransferCancelled implements Exception {
  const HistoryFileTransferCancelled();
}

/// Materializes bytes already received by the shared sync service for use by
/// a file action. Progress is measured from the actual destination file.
class HistoryFileTransfer {
  HistoryFileTransfer(
    this._repository, {
    Future<Directory> Function()? temporaryDirectory,
  }) : _temporaryDirectory = temporaryDirectory ?? getTemporaryDirectory;

  final HistoryRepository _repository;
  final Future<Directory> Function() _temporaryDirectory;
  final Map<String, Future<String>> _inFlight = {};
  Future<void>? _cleanup;
  static const _maxAge = Duration(minutes: 10);

  Future<String> prepare(
    HistoryClip clip, {
    required void Function(double) onProgress,
  }) {
    final pending = _inFlight[clip.id];
    if (pending != null) return pending;
    final operation = _prepare(clip, onProgress);
    _inFlight[clip.id] = operation;
    operation.whenComplete(() => _inFlight.remove(clip.id)).ignore();
    return operation;
  }

  Future<String> _prepare(
    HistoryClip clip,
    void Function(double) onProgress,
  ) async {
    final details = clip.file;
    if (details == null) throw StateError('The clip is not a file.');
    final root = await _temporaryDirectory();
    final directory = Directory('${root.path}/copypaste-file-transfers');
    await directory.create(recursive: true);
    await (_cleanup ??= _removeExpiredFiles(directory));
    final id = clip.id.replaceAll(RegExp('[^A-Za-z0-9_-]'), '_');
    final name = (details.name ?? 'file')
        .split(RegExp(r'[/\\]'))
        .last
        .replaceAll(RegExp('[^A-Za-z0-9._ -]'), '_');
    final destination = File(
      '${directory.path}/$id-${name.isEmpty ? 'file' : name}',
    );
    if (await destination.exists()) {
      final stat = await destination.stat();
      if (stat.type == FileSystemEntityType.file &&
          DateTime.now().difference(stat.modified) < _maxAge &&
          (details.sizeBytes == null || stat.size == details.sizeBytes)) {
        onProgress(1);
        return destination.path;
      }
      await destination.delete();
    }

    try {
      await saveTo(clip, destination.path, onProgress: onProgress);
      if (details.sizeBytes != null &&
          await destination.length() != details.sizeBytes) {
        throw StateError('The saved file is incomplete.');
      }
    } catch (_) {
      if (await destination.exists()) await destination.delete();
      rethrow;
    }
    return destination.path;
  }

  /// Saves to a user-selected destination and reports the bytes actually
  /// written by the existing runtime SaveFile operation.
  Future<void> saveTo(
    HistoryClip clip,
    String destinationPath, {
    required void Function(double) onProgress,
    bool Function()? isCancelled,
  }) async {
    final destination = File(destinationPath);
    final total = clip.file?.sizeBytes;
    var lastPercent = -1;
    void report(double value) {
      final percent = (value * 100).floor();
      if (percent == lastPercent) return;
      lastPercent = percent;
      onProgress(value);
    }

    report(0);
    var reading = false;
    var finished = false;
    final timer = Timer.periodic(const Duration(milliseconds: 40), (_) async {
      if (reading || finished) return;
      reading = true;
      try {
        if (total != null && total > 0 && await destination.exists()) {
          final written = await destination.length();
          if (!finished) report((written / total).clamp(0, 0.99));
        }
      } on FileSystemException {
        // The writer may be creating or rolling back the destination.
      } finally {
        reading = false;
      }
    });
    try {
      await _repository.saveFile(clip.id, destination.path);
      if (isCancelled?.call() ?? false) {
        if (await destination.exists()) await destination.delete();
        throw const HistoryFileTransferCancelled();
      }
      finished = true;
      report(1);
    } finally {
      finished = true;
      timer.cancel();
    }
  }

  Future<void> discard(String path) async {
    final file = File(path);
    if (await file.exists()) await file.delete();
  }

  Future<void> _removeExpiredFiles(Directory directory) async {
    final cutoff = DateTime.now().subtract(_maxAge);
    await for (final entry in directory.list(followLinks: false)) {
      if (entry is! File) continue;
      try {
        if ((await entry.stat()).modified.isBefore(cutoff)) {
          await entry.delete();
        }
      } on FileSystemException {
        // A concurrent OS cleanup must not prevent a new transfer.
      }
    }
  }
}
