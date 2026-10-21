import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:path_provider/path_provider.dart';
import 'package:crypto/crypto.dart';

import '../../../platform/files/temporary_artifacts.dart';

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
  }) : _temporaryDirectory =
           temporaryDirectory ?? getApplicationSupportDirectory;

  final HistoryRepository _repository;
  final Future<Directory> Function() _temporaryDirectory;
  final Map<HistoryClip, Future<String>> _inFlight = {};
  (HistoryClip, String)? _prepared;
  static const _maxAge = Duration(minutes: 10);

  Future<String> prepare(
    HistoryClip clip, {
    required void Function(double) onProgress,
  }) {
    final pending = _inFlight[clip];
    if (pending != null) return pending;
    final operation = _prepare(clip, onProgress);
    _inFlight[clip] = operation;
    operation.whenComplete(() => _inFlight.remove(clip)).ignore();
    return operation;
  }

  Future<String> _prepare(
    HistoryClip clip,
    void Function(double) onProgress,
  ) async {
    final details = clip.file;
    if (details == null) throw StateError('The clip is not a file.');
    if (_repository case final HistoryDownloadedFileRepository repository) {
      final existing = await repository.downloadedFile(clip.id);
      if (existing != null && await File(existing).exists()) {
        onProgress(1);
        return existing;
      }
      final root = await _temporaryDirectory();
      final downloads = Directory('${root.path}/copypaste-downloads');
      await downloads.create(recursive: true);
      final destination = await downloads.createTemp('file-');
      final name = _safeFileName(details.name);
      final path = '${destination.path}/${name.isEmpty ? 'file' : name}';
      // The runtime may still be writing after a request or presentation fails.
      // It owns partial-file cleanup; keep its directory available for retry.
      await saveTo(clip, path, onProgress: onProgress);
      return path;
    }
    final root = await _temporaryDirectory();
    final directory = Directory('${root.path}/copypaste-file-transfers');
    await directory.create(recursive: true);
    final previous = _prepared;
    await _removeExpiredFiles(directory, protectedPath: previous?.$2);
    if (previous != null && identical(previous.$1, clip)) {
      final cached = File(previous.$2);
      if (await cached.exists() &&
          DateTime.now().difference((await cached.stat()).modified) < _maxAge) {
        await cached.setLastModified(DateTime.now());
        onProgress(1);
        return cached.path;
      }
    }

    // IDs can be updated by sync. Materialize a newly observed clip, then name
    // its immutable cache file by its actual bytes, never by id and size alone.
    final lease = await TemporaryDirectoryLease.create(
      root,
      'copypaste-transfer-',
    );
    final staging = lease.directory;
    final staged = File('${staging.path}/payload');
    try {
      await saveTo(clip, staged.path, onProgress: onProgress);
      if (details.sizeBytes != null &&
          await staged.length() != details.sizeBytes) {
        throw StateError('The saved file is incomplete.');
      }
      final digest = (await sha256.bind(staged.openRead()).first).toString();
      final id = sha256
          .convert(utf8.encode(clip.id))
          .toString()
          .substring(0, 16);
      final name = _safeFileName(details.name);
      var destination = File(
        '${directory.path}/$id-$digest-${name.isEmpty ? 'file' : name}',
      );
      if (await destination.exists() &&
          (await sha256.bind(destination.openRead()).first).toString() !=
              digest) {
        // An invalid old cache must not replace bytes another consumer may
        // still have open. Publish this complete original under a unique name.
        destination = File(
          '${destination.path}-${staging.path.split(Platform.pathSeparator).last}',
        );
      }
      try {
        await staged.rename(destination.path);
      } on FileSystemException {
        // Windows can refuse replacement while another engine/consumer uses
        // this path. Reuse it only after verifying the actual content hash.
        if (!await destination.exists() ||
            (await sha256.bind(destination.openRead()).first).toString() !=
                digest) {
          rethrow;
        }
      }
      await destination.setLastModified(DateTime.now());
      _prepared = (clip, destination.path);
      return destination.path;
    } finally {
      await lease.dispose();
    }
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
    if (_repository case final HistoryFileProgressRepository repository) {
      final handled = await repository.saveFileWithProgress(
        clip.id,
        destination.path,
        (written, total) {
          if (total > 0) report((written / total).clamp(0, 0.99));
        },
      );
      if (handled) {
        // A backend with native cancellation owns the terminal outcome and
        // cleanup. A late UI flag must not delete its committed destination.
        if (_repository is! HistoryFileDownloadCancellation &&
            (isCancelled?.call() ?? false)) {
          if (await destination.exists()) await destination.delete();
          throw const HistoryFileTransferCancelled();
        }
        report(1);
        return;
      }
    }
    if (isCancelled?.call() ?? false) {
      throw const HistoryFileTransferCancelled();
    }
    var reading = false;
    var finished = false;
    final timer = Timer.periodic(const Duration(milliseconds: 40), (_) async {
      if (reading || finished) return;
      reading = true;
      try {
        if (total != null && total > 0) {
          final stat = await destination.stat();
          if (!finished && stat.type == FileSystemEntityType.file) {
            report((stat.size / total).clamp(0, 0.99));
          }
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

  String _safeFileName(String? value) {
    var name = (value ?? 'file')
        .split(RegExp(r'[/\\]'))
        .last
        .replaceAll(RegExp('[^A-Za-z0-9._ -]'), '_');
    if (name.length > 100) {
      final dot = name.lastIndexOf('.');
      final extension = dot > 0 && name.length - dot <= 16
          ? name.substring(dot)
          : '';
      name = '${name.substring(0, 100 - extension.length)}$extension';
    }
    return name.isEmpty ? 'file' : name;
  }

  Future<void> discard(String path) async {
    final file = File(path);
    if (await file.exists()) await file.delete();
  }

  Future<void> _removeExpiredFiles(
    Directory directory, {
    String? protectedPath,
  }) async {
    final cutoff = DateTime.now().subtract(const Duration(days: 1));
    await for (final entry in directory.list(followLinks: false)) {
      if (entry is! File || entry.path == protectedPath) continue;
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
