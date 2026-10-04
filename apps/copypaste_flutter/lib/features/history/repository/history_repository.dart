import '../models/history_models.dart';

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
  Future<HistoryImagePreview?> imagePreview(String id, {int? maxEdge});
  Future<HistorySourceAppIcon?> sourceAppIcon(String id);
  Future<void> copy(String id);
  Future<void> copyPlainText(String id);
  Future<void> saveFile(String id, String destinationPath);
  Future<void> setPinned(String id, bool pinned);
  Future<void> delete(String id);
  Future<void> deleteAll();
  Future<void> reorderPinned(List<String> ids);
}
