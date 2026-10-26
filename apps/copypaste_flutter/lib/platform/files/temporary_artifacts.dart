import 'dart:io';

import 'package:path_provider/path_provider.dart';

/// A lease protecting long-running OCR and downloads from age-based cleanup.
class TemporaryDirectoryLease {
  TemporaryDirectoryLease._(this.directory, this._lock);

  final Directory directory;
  RandomAccessFile? _lock;
  static const _marker = '.copypaste-active';

  static Future<TemporaryDirectoryLease> create(
    Directory root,
    String prefix,
  ) async {
    final staging = Directory('${root.path}/copypaste-staging');
    await staging.create(recursive: true);
    return acquire(await staging.createTemp(prefix));
  }

  static Future<TemporaryDirectoryLease> acquire(Directory directory) async {
    final lock = await File(
      '${directory.path}/$_marker',
    ).open(mode: FileMode.append);
    try {
      await lock.lock();
      // POSIX advisory locks are process-wide, so another Flutter engine in
      // this process must skip the directory without opening the lock file.
      await File('${directory.path}/.copypaste-owner').writeAsString('$pid');
      return TemporaryDirectoryLease._(directory, lock);
    } catch (_) {
      await lock.close();
      rethrow;
    }
  }

  Future<void> dispose() async {
    final lock = _lock;
    if (lock == null) return;
    _lock = null;
    await lock.close();
    if (await directory.exists()) await directory.delete(recursive: true);
  }
}

/// Cleans only abandoned application-owned staging directories. Installed
/// models, queued module updates, clipboard URIs and user files are excluded.
class TemporaryArtifactCleanup {
  TemporaryArtifactCleanup({
    Future<Directory> Function()? temporaryDirectory,
    DateTime Function()? now,
  }) : _temporaryDirectory = temporaryDirectory ?? getTemporaryDirectory,
       _now = now ?? DateTime.now;

  final Future<Directory> Function() _temporaryDirectory;
  final DateTime Function() _now;
  Future<void>? _running;
  DateTime? _lastRun;
  static const _prefixes = [
    'copypaste-ocr-',
    'copypaste-input-',
    'copypaste-transfer-',
    'module-download-',
    'module-catalog-',
  ];

  Future<void> run() {
    final pending = _running;
    if (pending != null) return pending;
    final now = _now();
    if (_lastRun != null &&
        now.difference(_lastRun!) < const Duration(hours: 1)) {
      return Future.value();
    }
    final operation = _sweep(now);
    _running = operation;
    return operation.whenComplete(() {
      _running = null;
    });
  }

  Future<void> _sweep(DateTime now) async {
    try {
      final root = await _temporaryDirectory();
      if (!await root.exists()) return;
      final cutoff = now.subtract(const Duration(days: 1));
      final staging = Directory('${root.path}/copypaste-staging');
      // Windows temporaryDirectory is shared with other applications. Legacy
      // unmarked generic module-* directories there cannot prove ownership.
      final roots = [
        if (await FileSystemEntity.type(staging.path, followLinks: false) ==
            FileSystemEntityType.directory)
          staging,
        if (Platform.isAndroid) root,
      ];
      for (final ownedRoot in roots) {
        await for (final entry in ownedRoot.list(followLinks: false)) {
          final name = entry.path.split(Platform.pathSeparator).last;
          if (entry is! Directory || !_prefixes.any(name.startsWith)) continue;
          try {
            if (!(await entry.stat()).modified.isBefore(cutoff)) continue;
            final marker = File(
              '${entry.path}/${TemporaryDirectoryLease._marker}',
            );
            final owner = File('${entry.path}/.copypaste-owner');
            if (await owner.exists()) {
              if (await owner.length() > 32) continue;
              final ownerPid = int.tryParse(await owner.readAsString());
              if (ownerPid == null || ownerPid == pid) continue;
            }
            // Legacy directories have no lease marker and predate this process.
            RandomAccessFile? lock;
            try {
              lock = await marker.open(mode: FileMode.append);
              await lock.lock();
            } on FileSystemException {
              await lock?.close();
              continue;
            }
            // Close before deleting for Windows, where an open file prevents
            // directory removal. Unique staging directory names are never reused.
            await lock.close();
            await entry.delete(recursive: true);
          } on FileSystemException {
            // Concurrent owner/OS cleanup must not block application startup.
          }
        }
      }
      _lastRun = now;
    } catch (_) {
      // Retry on the next lifecycle event, without a background polling timer.
    }
  }
}
