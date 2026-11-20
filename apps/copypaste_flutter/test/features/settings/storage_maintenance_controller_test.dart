import 'package:copypaste_flutter/features/settings/controller/storage_maintenance_controller.dart';
import 'package:copypaste_flutter/features/settings/models/storage_maintenance_models.dart';
import 'package:copypaste_flutter/features/settings/repository/storage_maintenance_repository.dart';
import 'package:copypaste_flutter/platform/files/folder_reveal.dart';
import 'package:copypaste_flutter/platform/lifecycle/local_data_eraser.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('clears caches without restoring and opens the logs folder', () async {
    final repository = _Repository();
    final folders = _Folders();
    var decodedCleared = 0;
    final controller = StorageMaintenanceController(
      repository: repository,
      folders: folders,
      clearDecodedImages: () => decodedCleared++,
    );
    addTearDown(controller.dispose);

    await controller.refresh();
    expect(controller.breakdown!.cacheBytes, 30);
    expect(controller.upgradeBackups.single.fromVersion, 10);

    expect(await controller.clearCache(), isTrue);
    expect(decodedCleared, 1);
    expect(controller.breakdown!.cacheBytes, 0);
    expect(controller.breakdown!.historyBytes, 100);
    expect(repository.restored, isEmpty);

    expect(await controller.openLogsFolder(), isTrue);
    expect(folders.revealed, ['/data/logs']);

    repository.failBreakdown = true;
    await controller.refresh();
    expect(controller.error, isNotNull);
  });

  test('unreadable backups keep the storage breakdown', () async {
    final repository = _Repository()..failBackups = true;
    final controller = StorageMaintenanceController(
      repository: repository,
      folders: _Folders(),
    );
    addTearDown(controller.dispose);

    await controller.refresh();

    expect(controller.error, isNull);
    expect(controller.breakdown!.cacheBytes, 30);
    expect(controller.upgradeBackups, isEmpty);
  });

  test('erasing all data reports a failure instead of exiting', () async {
    final eraser = _Eraser();
    final controller = StorageMaintenanceController(
      repository: _Repository(),
      eraser: eraser,
    );
    addTearDown(controller.dispose);

    expect(await controller.eraseAllData(), isTrue);
    expect(eraser.calls, 1);

    eraser.fail = true;
    expect(await controller.eraseAllData(), isFalse);
    expect(eraser.calls, 2);
    expect(controller.error, isNotNull);
  });
}

class _Eraser implements LocalDataEraser {
  var calls = 0;
  var fail = false;

  @override
  Future<void> eraseAndExit() async {
    calls++;
    if (fail) throw StateError('locked');
  }
}

StorageBreakdown _breakdown({required int cache}) => StorageBreakdown(
  historyBytes: 100,
  fileBytes: 20,
  cacheBytes: cache,
  upgradeBackupBytes: 40,
  otherBytes: 10,
);

class _Repository implements StorageMaintenanceRepository {
  final restored = <String>[];
  var cache = 30;
  var failBreakdown = false;
  var failBackups = false;

  @override
  Future<StorageBreakdown> breakdown() async {
    if (failBreakdown) throw StateError('unreadable');
    return _breakdown(cache: cache);
  }

  @override
  Future<StorageBreakdown> clearCache() async {
    cache = 0;
    return _breakdown(cache: cache);
  }

  @override
  Future<String?> dataDirectory() async => '/data';

  @override
  Future<void> restore(UpgradeBackup backup) async => restored.add(backup.path);

  @override
  Future<List<UpgradeBackup>> upgradeBackups() async {
    if (failBackups) throw StateError('unreadable');
    return [
      UpgradeBackup(
        path: '/data/upgrade-backups/1-v10.copypaste-backup',
        createdAt: DateTime(2026, 11, 15),
        fromVersion: 10,
        sizeBytes: 40,
      ),
    ];
  }
}

class _Folders implements FolderReveal {
  final revealed = <String>[];

  @override
  bool get supported => true;

  @override
  Future<void> reveal(String path) async => revealed.add(path);
}
