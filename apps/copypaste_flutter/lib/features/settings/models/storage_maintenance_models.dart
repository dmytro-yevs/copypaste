/// Local storage split by what it holds, in bytes.
class StorageBreakdown {
  const StorageBreakdown({
    required this.historyBytes,
    required this.fileBytes,
    required this.cacheBytes,
    required this.upgradeBackupBytes,
    required this.otherBytes,
  });

  final int historyBytes;
  final int fileBytes;

  /// Decoded previews and temporary files; Clear cache removes them.
  final int cacheBytes;
  final int upgradeBackupBytes;

  /// Logs, modules, device and sync state.
  final int otherBytes;

  int get totalBytes =>
      historyBytes + fileBytes + cacheBytes + upgradeBackupBytes + otherBytes;
}

/// One encrypted copy kept before a data-format upgrade.
class UpgradeBackup {
  const UpgradeBackup({
    required this.path,
    required this.createdAt,
    required this.fromVersion,
    required this.sizeBytes,
  });

  final String path;
  final DateTime createdAt;
  final int fromVersion;
  final int sizeBytes;
}

enum StorageMaintenanceOperation {
  refresh,
  clearCache,
  restore,
  openFolder,
  eraseAll,
}
