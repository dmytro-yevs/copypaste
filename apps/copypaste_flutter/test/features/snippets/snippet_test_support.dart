import 'dart:async';

import 'package:copypaste_flutter/features/snippets/models/snippet_models.dart';
import 'package:copypaste_flutter/features/snippets/repository/snippet_repository.dart';

SnippetSummary summary(
  String id, {
  String title = '',
  String? abbreviation,
  bool pinned = false,
  bool concealed = false,
  String preview = 'text',
}) => SnippetSummary(
  id: id,
  title: title,
  abbreviation: abbreviation,
  pinned: pinned,
  concealed: concealed,
  preview: preview,
  createdAt: DateTime(2026),
);

/// An in-memory [SnippetRepository] that records the writes it receives.
class FakeSnippetRepository implements SnippetRepository {
  FakeSnippetRepository({
    List<SnippetSummary> snippets = const [],
    this.details = const {},
    this.preferencesValue = const SnippetPreferences(),
  }) : snippets = [...snippets];

  final List<SnippetSummary> snippets;
  final Map<String, SnippetDetail> details;
  SnippetPreferences preferencesValue;
  final saved = <SnippetDraft>[];
  final used = <(String, Map<String, String>, SnippetDelivery)>[];
  final renamed = <(String, String, bool)>[];
  int releases = 0;
  final _changes = StreamController<void>.broadcast();

  @override
  Future<SnippetPage> list(
    SnippetListScope scope, {
    String? cursor,
    int limit = SnippetLimits.pageSize,
  }) async {
    final rows = [
      for (final row in snippets)
        if (switch (scope.kind) {
          SnippetScopeKind.pinned => row.pinned && !row.deleted,
          SnippetScopeKind.recentlyDeleted => row.deleted,
          _ => !row.deleted,
        })
          row,
    ];
    return SnippetPage(snippets: rows, total: rows.length);
  }

  @override
  Future<SnippetDetail> get(String id) async =>
      details[id] ??
      SnippetDetail(
        summary: snippets.firstWhere((row) => row.id == id),
        bodyText: 'text',
      );

  @override
  Future<SnippetSaveOutcome> save(SnippetDraft draft) async {
    saved.add(draft);
    final id = draft.id ?? 'new-${saved.length}';
    return SnippetSaved(
      SnippetDetail(
        summary: SnippetSummary(
          id: id,
          title: draft.title,
          abbreviation: draft.abbreviation,
          createdAt: DateTime(2026),
        ),
        bodyText: draft.bodyText,
      ),
    );
  }

  @override
  Future<int> renameAbbreviation(
    String id,
    String abbreviation, {
    required bool updateReferences,
  }) async {
    renamed.add((id, abbreviation, updateReferences));
    return 0;
  }

  @override
  Future<List<SnippetHit>> search(String query, {int limit = 50}) async => [
    for (final row in snippets)
      if (row.abbreviation == query)
        SnippetHit(snippet: row, tier: SnippetMatchTier.exactAbbreviation),
  ];

  @override
  Future<List<TemplateLint>> lint(
    String body, {
    required bool template,
  }) async => const [];

  @override
  Future<RenderedTemplate> render({
    String? snippetId,
    String? body,
    Map<String, String> values = const {},
    TemplateClipboardSource clipboard = TemplateClipboardSource.none,
    bool finalRender = false,
  }) async => RenderedTemplate(text: values.values.join(' '));

  @override
  Future<SnippetDelivered> use(
    String id, {
    Map<String, String> values = const {},
    required SnippetDelivery delivery,
  }) async {
    used.add((id, values, delivery));
    return const SnippetDelivered();
  }

  @override
  Future<void> release() async => releases++;

  @override
  Future<List<SnippetCollection>> collections() async => const [];

  @override
  Future<SnippetPreferences> preferences() async => preferencesValue;

  @override
  Future<SnippetPreferences> setPreference(
    SnippetPreference preference,
    bool value,
  ) async => preferencesValue = SnippetPreferences(
    recordPasted: preference == SnippetPreference.recordPasted
        ? value
        : preferencesValue.recordPasted,
    requireAuth: preference == SnippetPreference.requireAuth
        ? value
        : preferencesValue.requireAuth,
  );

  @override
  Stream<void> changes() => _changes.stream;

  @override
  Future<SnippetDetail> saveFromClip(String clipId, {String? collectionId}) =>
      throw UnimplementedError();

  @override
  Future<void> delete(List<String> ids) async {}

  @override
  Future<void> restore(List<String> ids) async {}

  @override
  Future<void> purge(List<String> ids) async {}

  @override
  Future<void> move(List<String> ids, String? collectionId) async {}

  @override
  Future<void> setPinned(String id, {required bool pinned}) async {}

  @override
  Future<SnippetCollection> saveCollection(SnippetCollectionDraft draft) =>
      throw UnimplementedError();

  @override
  Future<void> deleteCollection(
    String id, {
    required bool deleteSnippets,
  }) async {}

  @override
  Future<void> mergeCollections(String from, String into) async {}

  @override
  Future<SnippetImportPlan> previewImport(
    String path, {
    required SnippetImportFormat format,
    String? passphrase,
    bool convertPlaceholders = false,
  }) => throw UnimplementedError();

  @override
  Future<SnippetImportReport> commitImport(
    String token,
    SnippetImportPolicies policies,
  ) => throw UnimplementedError();

  @override
  Future<void> cancelImport(String token) async {}

  @override
  Future<int> export(SnippetExportRequest request) async => 0;
}
