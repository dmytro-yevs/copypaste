import '../models/snippet_models.dart';

/// Runtime access for the snippet library, templates and transfer.
///
/// Failures are [SnippetException]s carrying the runtime error code.
abstract interface class SnippetRepository {
  Future<SnippetPage> list(
    SnippetListScope scope, {
    String? cursor,
    int limit = SnippetLimits.pageSize,
  });

  Future<SnippetDetail> get(String id);

  Future<SnippetSaveOutcome> save(SnippetDraft draft);

  /// A literal snippet with a clip's text; the clip stays unchanged.
  Future<SnippetDetail> saveFromClip(String clipId, {String? collectionId});

  /// Moves to Recently Deleted.
  Future<void> delete(List<String> ids);

  Future<void> restore(List<String> ids);

  /// Permanently erases the content everywhere.
  Future<void> purge(List<String> ids);

  /// [collectionId] null moves to Unfiled.
  Future<void> move(List<String> ids, String? collectionId);

  Future<void> setPinned(String id, {required bool pinned});

  /// Renames an abbreviation; [updateReferences] rewrites the templates that
  /// name the old one. Returns how many references were rewritten.
  Future<int> renameAbbreviation(
    String id,
    String abbreviation, {
    required bool updateReferences,
  });

  Future<List<SnippetCollection>> collections();

  Future<SnippetCollection> saveCollection(SnippetCollectionDraft draft);

  Future<void> deleteCollection(String id, {required bool deleteSnippets});

  Future<void> mergeCollections(String from, String into);

  Future<List<TemplateLint>> lint(String body, {required bool template});

  /// Ranked snippet matches: exact abbreviation, prefix, title, full text.
  Future<List<SnippetHit>> search(String query, {int limit = 50});

  /// Renders a saved snippet ([snippetId]) or an unsaved [body].
  Future<RenderedTemplate> render({
    String? snippetId,
    String? body,
    Map<String, String> values = const {},
    TemplateClipboardSource clipboard = TemplateClipboardSource.none,
    bool finalRender = false,
  });

  Future<SnippetDelivered> use(
    String id, {
    Map<String, String> values = const {},
    required SnippetDelivery delivery,
  });

  /// Drops the template cache and any import plan; sent when a surface closes.
  Future<void> release();

  Future<SnippetImportPlan> previewImport(
    String path, {
    required SnippetImportFormat format,
    String? passphrase,
    bool convertPlaceholders = false,
  });

  Future<SnippetImportReport> commitImport(
    String token,
    SnippetImportPolicies policies,
  );

  Future<void> cancelImport(String token);

  /// Returns how many snippets were written.
  Future<int> export(SnippetExportRequest request);

  Future<SnippetPreferences> preferences();

  Future<SnippetPreferences> setPreference(
    SnippetPreference preference,
    bool value,
  );

  /// Signals that snippets, collections or preferences changed, here or on
  /// a peer.
  Stream<void> changes();
}

/// Text clip creation and editing for History.
abstract interface class ClipEditRepository {
  /// A new history clip attributed to CopyPaste with a manual marker.
  Future<String> createManualClip(String text);

  /// Saves [text] as a new clip marked edited; [replaceOriginal] deletes the
  /// original after the new clip is saved. Returns the resulting clip id.
  Future<String> saveClipEdit(
    String id,
    String text, {
    required bool replaceOriginal,
  });
}
