import 'dart:io';

import 'package:copypaste_flutter/generated/api/storage.dart' as runtime;
import 'package:copypaste_flutter/generated/api.dart' as runtime_api;

import '../../../platform/files/runtime_data_directory.dart';
import '../../../platform/files/temporary_artifacts.dart';
import '../models/storage_maintenance_models.dart';

abstract interface class StorageMaintenanceRepository {
  Future<StorageBreakdown> breakdown();

  /// Clears every cache and returns the storage that remains.
  Future<StorageBreakdown> clearCache();

  Future<List<UpgradeBackup>> upgradeBackups();

  Future<void> restore(UpgradeBackup backup);

  /// Where history, logs and backups live; null where it cannot be shown.
  Future<String?> dataDirectory();
}

class RuntimeStorageMaintenanceRepository
    implements StorageMaintenanceRepository {
  RuntimeStorageMaintenanceRepository({TemporaryArtifactCleanup? staging})
    : _staging = staging ?? TemporaryArtifactCleanup();

  final TemporaryArtifactCleanup _staging;

  @override
  Future<StorageBreakdown> breakdown() async =>
      _withStaging(await runtime.storageBreakdown(), await _staging.measure());

  @override
  Future<StorageBreakdown> clearCache() async {
    final staging = await _staging.clear();
    return _withStaging(await runtime.clearStorageCache(), staging.after);
  }

  @override
  Future<List<UpgradeBackup>> upgradeBackups() async => [
    for (final backup in await runtime.upgradeBackups())
      UpgradeBackup(
        path: backup.path,
        createdAt: DateTime.fromMillisecondsSinceEpoch(
          backup.createdAtMs.toInt(),
        ),
        fromVersion: backup.fromVersion,
        sizeBytes: backup.sizeBytes.toInt(),
      ),
  ];

  @override
  Future<void> restore(UpgradeBackup backup) =>
      runtime_api.restoreHistory(filePath: backup.path);

  @override
  Future<String?> dataDirectory() async =>
      Platform.isMacOS || Platform.isWindows
      ? (await runtimeDataDirectory()).path
      : null;

  StorageBreakdown _withStaging(runtime.StorageBreakdown value, int staging) =>
      StorageBreakdown(
        historyBytes: value.historyBytes.toInt(),
        fileBytes: value.fileBytes.toInt(),
        cacheBytes: value.cacheBytes.toInt() + staging,
        upgradeBackupBytes: value.upgradeBackupBytes.toInt(),
        otherBytes: value.otherBytes.toInt(),
      );
}
