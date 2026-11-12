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
  Future<void> copyText(String id, String text);
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

/// Reports native writer progress and completes when the save is committed.
abstract interface class HistoryFileProgressRepository {
  Future<void> saveFileWithProgress(
    String id,
    String destinationPath,
    void Function(int written, int total) onProgress,
  );
}

/// Reuses native-verified downloads by clip version across UI engines.
abstract interface class HistoryDownloadedFileRepository {
  Future<String?> downloadedFile(String id);
}

/// Exact group sizes for a list query, counted without loading its pages.
abstract interface class HistoryGroupCountsRepository {
  /// [days] are local calendar days. Null means the runtime cannot count this
  /// query exactly (text search or a kind filter); it never estimates.
  Future<HistoryGroupCounts?> groupCounts({
    required HistoryQuery query,
    required List<DateTime> days,
  });
}

/// Tag management is separate from paged payload access.
abstract interface class HistoryTagsRepository {
  Future<HistoryTags> tags(List<String> clipIds);
  Future<void> saveTag({String? id, required String name});
  Future<void> deleteTag(String id);
  Future<void> changeTags({
    required List<String> clipIds,
    required List<String> tagIds,
    required bool assigned,
  });
}
