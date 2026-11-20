import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/painting.dart';
import 'package:flutter/foundation.dart';

import '../../../platform/files/folder_reveal.dart';
import '../../../platform/lifecycle/local_data_eraser.dart';
import '../models/storage_maintenance_models.dart';
import '../repository/storage_maintenance_repository.dart';

/// Storage usage, cache clearing, pre-upgrade backups and the data folders.
class StorageMaintenanceController extends ChangeNotifier {
  StorageMaintenanceController({
    required StorageMaintenanceRepository repository,
    FolderReveal folders = const SystemFolderReveal(),
    LocalDataEraser eraser = const PlatformLocalDataEraser(),
    VoidCallback? clearDecodedImages,
  }) : _repository = repository,
       _folders = folders,
       _eraser = eraser,
       _clearDecodedImages = clearDecodedImages ?? _clearImageCache;

  final StorageMaintenanceRepository _repository;
  final FolderReveal _folders;
  final LocalDataEraser _eraser;
  final VoidCallback _clearDecodedImages;

  StorageBreakdown? _breakdown;
  List<UpgradeBackup> _upgradeBackups = const [];
  StorageMaintenanceOperation? _pending;
  String? _error;
  bool _disposed = false;

  StorageBreakdown? get breakdown => _breakdown;
  List<UpgradeBackup> get upgradeBackups => _upgradeBackups;
  String? get error => _error;
  bool get busy => _pending != null;
  bool isPending(StorageMaintenanceOperation operation) =>
      _pending == operation;
  bool get canOpenFolders => _folders.supported;

  Future<void> refresh() => _run(StorageMaintenanceOperation.refresh, () async {
    // Backups are optional: either read failing keeps the other one.
    final backups = _repository.upgradeBackups().then<List<UpgradeBackup>?>(
      (backups) => backups,
      onError: (Object _) => null,
    );
    try {
      _breakdown = await _repository.breakdown();
    } finally {
      if (await backups case final loaded?) _upgradeBackups = loaded;
    }
  }, AppStrings.current.settingsStorageUsageUnavailable);

  /// Previews, decoded images and abandoned temporary files; never history.
  Future<bool> clearCache() =>
      _run(StorageMaintenanceOperation.clearCache, () async {
        _clearDecodedImages();
        _breakdown = await _repository.clearCache();
      }, AppStrings.current.settingsClearCacheFailed);

  Future<bool> restore(UpgradeBackup backup) =>
      _run(StorageMaintenanceOperation.restore, () async {
        await _repository.restore(backup);
        _breakdown = await _repository.breakdown();
      }, AppStrings.current.settingsRestoreFailed);

  /// History, settings and the device key; the app exits when it succeeds.
  Future<bool> eraseAllData() => _run(
    StorageMaintenanceOperation.eraseAll,
    _eraser.eraseAndExit,
    AppStrings.current.settingsEraseAllDataFailed,
  );

  Future<bool> openDataFolder() => _openFolder(null);

  Future<bool> openLogsFolder() => _openFolder('logs');

  Future<bool> _openFolder(String? child) =>
      _run(StorageMaintenanceOperation.openFolder, () async {
        final root = await _repository.dataDirectory();
        if (root == null) throw UnsupportedError('No data folder to show.');
        await _folders.reveal(child == null ? root : '$root/$child');
      }, AppStrings.current.settingsOpenFolderFailed);

  Future<bool> _run(
    StorageMaintenanceOperation operation,
    Future<void> Function() action,
    String failure,
  ) async {
    if (_disposed || _pending != null) return false;
    _pending = operation;
    _error = null;
    notifyListeners();
    try {
      await action();
      return true;
    } catch (_) {
      _error = failure;
      return false;
    } finally {
      _pending = null;
      if (!_disposed) notifyListeners();
    }
  }

  static void _clearImageCache() {
    final cache = PaintingBinding.instance.imageCache;
    cache.clear();
    cache.clearLiveImages();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
