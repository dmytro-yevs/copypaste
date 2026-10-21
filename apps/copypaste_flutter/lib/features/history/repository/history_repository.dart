import '../models/history_models.dart';
import 'history_file_importer.dart';

/// Consumer-facing port over generated Rust bindings.
///
/// The production adapter is the only place that imports generated/api.dart.
abstract interface class HistoryRepository {
  Stream<HistoryRuntimeEvent> watch();

  Future<HistoryFacets> facets();

  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  });

  Future<HistoryClip> get(String id);
  Future<HistoryImagePreview?> imagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  });
  Future<HistorySourceAppIcon?> sourceAppIcon(String id);
  Future<void> copy(String id);
  Future<void> copyPlainText(String id);
  Future<void> saveFile(String id, String destinationPath);
  Future<void> importFile(HistoryImportFile file);
  Future<HistoryClip> setSecret(String id, bool secret);
  Future<void> setPinned(String id, bool pinned);
  Future<void> delete(String id);
  Future<void> deleteAll();
  Future<void> reorderPinned(List<String> ids);
}

abstract interface class HistoryFileDownloadCancellation {
  Future<void> cancelFileDownload(String id);
}

/// Uses native writer events when the backend supports them. Returns false
/// without starting a save when the legacy filesystem fallback is required.
abstract interface class HistoryFileProgressRepository {
  Future<bool> saveFileWithProgress(
    String id,
    String destinationPath,
    void Function(int written, int total) onProgress,
  );
}

/// Reuses native-verified downloads by clip version across UI engines.
abstract interface class HistoryDownloadedFileRepository {
  Future<String?> downloadedFile(String id);
}
