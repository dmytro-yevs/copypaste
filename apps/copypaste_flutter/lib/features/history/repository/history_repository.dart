import '../../content_rules/models/content_rule_models.dart';
import 'clip_edit_preferences.dart';
import 'clip_edit_repository.dart';
import '../../transforms/repository/transform_recents_store.dart';
import '../../transforms/repository/transform_repository.dart';
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

/// Editing clips and writing new ones.
abstract interface class HistoryEditRepository {
  ClipEditRepository get clipEdits;
  ClipEditPreferences get clipEditPreferences;
}

/// Adding History clips to this device's paste queue.
abstract interface class HistoryQueueRepository {
  /// Appends [clipIds] in the given order and returns how many entries are
  /// now pending. Throws when the queue is turned off or refuses the add.
  Future<int> addToQueue(List<String> clipIds);

  /// The text Paste all puts between entries.
  Future<String> pasteAllSeparator();
}

/// A paired device History clips can be sent to.
class HistorySendTarget {
  const HistorySendTarget({required this.id, required this.name});

  /// The pairing id.
  final String id;
  final String name;
}

/// One device's answer to a manual send.
class HistorySendResult {
  const HistorySendResult({
    required this.name,
    required this.sent,
    this.errorCode,
  });

  final String name;

  /// Clips the device lacked and took; zero when it already had them.
  final int sent;

  /// `unsupported_content` when the sync settings refuse the clips.
  final String? errorCode;
}

/// Sending History clips to paired devices now, even while automatic sync is
/// paused.
abstract interface class HistorySendRepository {
  /// Paired devices that a completed sync has identified.
  Future<List<HistorySendTarget>> sendTargets();
  Future<List<HistorySendResult>> sendClips(
    List<String> clipIds,
    List<String> targetIds,
  );
}

/// The transform palette's ports for History clips.
abstract interface class HistoryTransformRepository {
  TransformRepository get transforms;
  TransformRecentsStore get transformRecents;
}

/// Per-clip annotations: keeping an expiring clip and the content-rule trace
/// that explains a clip's state.
abstract interface class HistoryAnnotationsRepository {
  /// Clears any expiry and returns the clip's updated annotations.
  Future<HistoryClipAnnotations> keep(String clipId);

  Future<RuleTrace> ruleTrace(String clipId);

  /// Names of the tags and transforms a trace refers to.
  Future<RuleOptions> ruleOptions();
}
