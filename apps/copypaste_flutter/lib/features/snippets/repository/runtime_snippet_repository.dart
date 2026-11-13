import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/generated/api/clips.dart' as clips;
import 'package:copypaste_flutter/generated/api/snippets.dart' as api;

import '../../../l10n/l10n.dart';
import '../../../shared/runtime_event_service.dart';
import '../models/snippet_models.dart';
import 'snippet_repository.dart';

/// [SnippetRepository] and [ClipEditRepository] over the runtime bridge.
class RuntimeSnippetRepository
    implements SnippetRepository, ClipEditRepository {
  RuntimeSnippetRepository({RuntimeEventService? events})
    : _events = events ?? RuntimeEventService.instance;

  final RuntimeEventService _events;

  static String get _locale => AppStrings.current.localeName;

  @override
  Future<SnippetPage> list(
    SnippetListScope scope, {
    String? cursor,
    int limit = SnippetLimits.pageSize,
  }) => _guard(() async {
    final page = await api.listSnippets(
      scope: switch (scope.kind) {
        SnippetScopeKind.all => api.SnippetScopeKind.all,
        SnippetScopeKind.unfiled => api.SnippetScopeKind.unfiled,
        SnippetScopeKind.collection => api.SnippetScopeKind.collection,
        SnippetScopeKind.pinned => api.SnippetScopeKind.pinned,
        SnippetScopeKind.recentlyDeleted => api.SnippetScopeKind.recentlyDeleted,
      },
      collectionId: scope.collectionId,
      cursor: cursor,
      limit: limit,
    );
    return SnippetPage(
      snippets: page.snippets.map(_summary).toList(growable: false),
      nextCursor: page.nextCursor,
      total: page.total,
    );
  });

  @override
  Future<SnippetDetail> get(String id) =>
      _guard(() async => _detail(await api.getSnippet(id: id)));

  @override
  Future<SnippetSaveOutcome> save(SnippetDraft draft) => _guard(() async {
    final result = await api.saveSnippet(
      draft: api.SnippetDraftInput(
        id: draft.id,
        collectionId: draft.collectionId,
        title: draft.title,
        abbreviation: draft.abbreviation,
        bodyText: draft.bodyText,
        bodyRich: draft.bodyRich,
        template: draft.template,
        pinned: draft.pinned,
        concealed: draft.concealed,
        baseRevision: draft.baseRevision,
      ),
    );
    if (result.saved case final saved?) return SnippetSaved(_detail(saved));
    return SnippetAbbreviationTaken(
      snippetId: result.abbreviationTakenById ?? '',
      title: result.abbreviationTakenByTitle ?? '',
    );
  });

  @override
  Future<SnippetDetail> saveFromClip(String clipId, {String? collectionId}) =>
      _guard(
        () async => _detail(
          await api.saveSnippetFromClip(
            clipId: clipId,
            collectionId: collectionId,
          ),
        ),
      );

  @override
  Future<void> delete(List<String> ids) =>
      _guard(() => api.deleteSnippets(ids: ids));

  @override
  Future<void> restore(List<String> ids) =>
      _guard(() => api.restoreSnippets(ids: ids));

  @override
  Future<void> purge(List<String> ids) =>
      _guard(() => api.purgeSnippets(ids: ids));

  @override
  Future<void> move(List<String> ids, String? collectionId) =>
      _guard(() => api.moveSnippets(ids: ids, collectionId: collectionId));

  @override
  Future<void> setPinned(String id, {required bool pinned}) =>
      _guard(() => api.setSnippetPinned(id: id, pinned: pinned));

  @override
  Future<int> renameAbbreviation(
    String id,
    String abbreviation, {
    required bool updateReferences,
  }) => _guard(
    () async => (await api.renameSnippetAbbreviation(
      id: id,
      abbreviation: abbreviation,
      updateReferences: updateReferences,
    )).toInt(),
  );

  @override
  Future<List<SnippetCollection>> collections() => _guard(
    () async => [
      for (final collection in await api.snippetCollections())
        if (collection.deletedAtMs == null) _collection(collection),
    ],
  );

  @override
  Future<SnippetCollection> saveCollection(SnippetCollectionDraft draft) =>
      _guard(
        () async => _collection(
          await api.saveSnippetCollection(
            draft: api.SnippetCollectionDraft(
              id: draft.id,
              name: draft.name,
              icon: draft.icon.name,
              accent: draft.accent.name,
            ),
          ),
        ),
      );

  @override
  Future<void> deleteCollection(String id, {required bool deleteSnippets}) =>
      _guard(
        () =>
            api.deleteSnippetCollection(id: id, deleteSnippets: deleteSnippets),
      );

  @override
  Future<void> mergeCollections(String from, String into) =>
      _guard(() => api.mergeSnippetCollections(from: from, into: into));

  @override
  Future<List<TemplateLint>> lint(String body, {required bool template}) =>
      _guard(
        () async => (await api.lintSnippetBody(
          body: body,
          template: template,
        )).map(_lint).toList(growable: false),
      );

  @override
  Future<List<SnippetHit>> search(String query, {int limit = 50}) => _guard(
    () async => [
      for (final hit in await api.quickSearchSnippets(
        query: query,
        includeSnippets: true,
        limit: limit,
      ))
        SnippetHit(
          snippet: _summary(hit.snippet),
          tier: switch (hit.tier) {
            api.SnippetMatchTier.exactAbbreviation =>
              SnippetMatchTier.exactAbbreviation,
            api.SnippetMatchTier.abbreviationPrefix =>
              SnippetMatchTier.abbreviationPrefix,
            api.SnippetMatchTier.title => SnippetMatchTier.title,
            api.SnippetMatchTier.fullText => SnippetMatchTier.fullText,
          },
          duplicate: hit.duplicate,
        ),
    ],
  );

  @override
  Future<RenderedTemplate> render({
    String? snippetId,
    String? body,
    Map<String, String> values = const {},
    TemplateClipboardSource clipboard = TemplateClipboardSource.none,
    bool finalRender = false,
  }) => _guard(() async {
    final rendered = await api.renderSnippet(
      snippetId: snippetId,
      body: body,
      values: _values(values),
      clipboard: _clipboard(clipboard),
      finalRender: finalRender,
      locale: _locale,
    );
    return RenderedTemplate(
      text: rendered.text,
      cursorFromEnd: rendered.cursorGraphemesFromEnd,
      warnings: rendered.warnings.map(_lint).toList(growable: false),
      missingRequired: rendered.missingRequired,
      unfilled: rendered.unfilled,
    );
  });

  @override
  Future<SnippetDelivered> use(
    String id, {
    Map<String, String> values = const {},
    required SnippetDelivery delivery,
  }) => _guard(() async {
    final delivered = await api.useSnippet(
      id: id,
      values: _values(values),
      clipboard: _clipboard(TemplateClipboardSource.host),
      delivery: switch (delivery) {
        SnippetDelivery.paste => clips.TextDelivery.paste,
        SnippetDelivery.copy => clips.TextDelivery.copy,
        SnippetDelivery.returnText => clips.TextDelivery.return_,
      },
      locale: _locale,
    );
    return SnippetDelivered(
      token: delivered.token,
      text: delivered.text,
      cursorFromEnd: delivered.cursorGraphemesFromEnd,
    );
  });

  @override
  Future<void> release() => _guard(api.releaseSnippetResources);

  @override
  Future<SnippetImportPlan> previewImport(
    String path, {
    required SnippetImportFormat format,
    String? passphrase,
    bool convertPlaceholders = false,
  }) => _guard(() async {
    final plan = await api.previewSnippetImport(
      path: path,
      passphrase: passphrase,
      format: switch (format) {
        SnippetImportFormat.bundle => api.SnippetImportFormat.bundle,
        SnippetImportFormat.json => api.SnippetImportFormat.json,
        SnippetImportFormat.csv => api.SnippetImportFormat.csv,
      },
      convertPlaceholders: convertPlaceholders,
    );
    final counts = plan.counts;
    return SnippetImportPlan(
      token: plan.token,
      counts: SnippetImportCounts(
        added: counts.newRows,
        identical: counts.identicalRows,
        updated: counts.updated,
        abbreviationConflicts: counts.abbreviationConflicts,
        invalid: counts.invalid,
        collections: counts.collections,
      ),
      rows: [
        for (final row in plan.rows)
          SnippetImportRow(
            index: row.index,
            outcome: switch (row.outcome) {
              api.SnippetImportRowOutcome.new_ => SnippetImportRowOutcome.added,
              api.SnippetImportRowOutcome.identical =>
                SnippetImportRowOutcome.identical,
              api.SnippetImportRowOutcome.updated =>
                SnippetImportRowOutcome.updated,
              api.SnippetImportRowOutcome.abbreviationConflict =>
                SnippetImportRowOutcome.abbreviationConflict,
              api.SnippetImportRowOutcome.invalid =>
                SnippetImportRowOutcome.invalid,
            },
            reason: row.reason,
          ),
      ],
    );
  });

  @override
  Future<SnippetImportReport> commitImport(
    String token,
    SnippetImportPolicies policies,
  ) => _guard(() async {
    final report = await api.commitSnippetImport(
      token: token,
      policies: api.SnippetImportPolicies(
        sameIdDifferent: switch (policies.sameIdDifferent) {
          SnippetSameIdPolicy.keepBoth => api.SnippetSameIdPolicy.keepBoth,
          SnippetSameIdPolicy.replace => api.SnippetSameIdPolicy.replace,
          SnippetSameIdPolicy.skip => api.SnippetSameIdPolicy.skip,
        },
        abbreviationConflict: switch (policies.abbreviationConflict) {
          SnippetAbbreviationPolicy.importWithout =>
            api.SnippetAbbreviationPolicy.importWithout,
          SnippetAbbreviationPolicy.renameWithSuffix =>
            api.SnippetAbbreviationPolicy.renameWithSuffix,
          SnippetAbbreviationPolicy.skip => api.SnippetAbbreviationPolicy.skip,
        },
        collectionName: switch (policies.collectionName) {
          SnippetCollectionNamePolicy.merge =>
            api.SnippetCollectionNamePolicy.merge,
          SnippetCollectionNamePolicy.createNew =>
            api.SnippetCollectionNamePolicy.createNew,
        },
      ),
    );
    return SnippetImportReport(
      inserted: report.inserted,
      updated: report.updated,
      skipped: report.skipped,
      renamedAbbreviations: report.renamedAbbreviations,
      withoutAbbreviation: report.withoutAbbreviation,
      collectionsCreated: report.collectionsCreated,
      unrecognizedPlaceholders: report.unrecognizedPlaceholders,
    );
  });

  @override
  Future<void> cancelImport(String token) =>
      _guard(() => api.cancelSnippetImport(token: token));

  @override
  Future<int> export(SnippetExportRequest request) => _guard(
    () async => (await api.exportSnippets(
      path: request.path,
      scope: switch (request.scope) {
        SnippetExportScopeKind.all => api.SnippetExportScopeKind.all,
        SnippetExportScopeKind.collections =>
          api.SnippetExportScopeKind.collections,
        SnippetExportScopeKind.snippets => api.SnippetExportScopeKind.snippets,
      },
      ids: request.ids,
      passphrase: request.passphrase,
      includeConcealed: request.includeConcealed,
    )).toInt(),
  );

  @override
  Future<SnippetPreferences> preferences() =>
      _guard(() async => _preferences(await api.snippetPreferences()));

  @override
  Future<SnippetPreferences> setPreference(
    SnippetPreference preference,
    bool value,
  ) => _guard(
    () async => _preferences(
      await api.setSnippetPreference(
        key: switch (preference) {
          SnippetPreference.recordPasted =>
            api.SnippetPreferenceKind.recordPasted,
          SnippetPreference.requireAuth =>
            api.SnippetPreferenceKind.requireAuth,
        },
        value: value,
      ),
    ),
  );

  @override
  Stream<void> changes() => _events.libraryChanges(const {
    LibraryKind.snippets,
    LibraryKind.collections,
    LibraryKind.preferences,
  });

  @override
  Future<String> createManualClip(String text) =>
      _guard(() async => (await clips.createManualClip(text: text)).id);

  @override
  Future<String> saveClipEdit(
    String id,
    String text, {
    required bool replaceOriginal,
  }) => _guard(
    () async => (await clips.saveClipEdit(
      id: id,
      text: text,
      replaceOriginal: replaceOriginal,
    )).id,
  );

  static Future<T> _guard<T>(Future<T> Function() request) async {
    try {
      return await request();
    } on runtime.RuntimeError catch (error) {
      throw SnippetException(error.code, error.message);
    }
  }

  static List<api.TemplateValue> _values(Map<String, String> values) => [
    for (final MapEntry(:key, :value) in values.entries)
      api.TemplateValue(name: key, value: value),
  ];

  static api.TemplateClipboard _clipboard(TemplateClipboardSource source) =>
      api.TemplateClipboard(
        kind: switch (source) {
          TemplateClipboardSource.host => api.TemplateClipboardKind.host,
          TemplateClipboardSource.none => api.TemplateClipboardKind.none,
        },
      );

  static DateTime _time(int ms) => DateTime.fromMillisecondsSinceEpoch(ms);

  static SnippetSummary _summary(api.SnippetSummaryView view) =>
      SnippetSummary(
        id: view.id,
        collectionId: view.collectionId,
        title: view.title,
        abbreviation: view.abbreviation,
        template: view.template,
        pinned: view.pinned,
        concealed: view.concealed,
        preview: view.preview,
        createdAt: _time(view.createdAtMs.toInt()),
        deletedAt: view.deletedAtMs == null
            ? null
            : _time(view.deletedAtMs!.toInt()),
        duplicateAbbreviation: view.duplicateAbbreviation,
        conflictCopy: view.conflictCopy,
        lastUsedAt: view.lastUsedAtMs == null
            ? null
            : _time(view.lastUsedAtMs!.toInt()),
      );

  static TemplateLint _lint(api.TemplateLintView view) => TemplateLint(
    severity: switch (view.severity) {
      api.TemplateLintSeverity.error => TemplateLintSeverity.error,
      api.TemplateLintSeverity.warning => TemplateLintSeverity.warning,
      api.TemplateLintSeverity.hint => TemplateLintSeverity.hint,
    },
    code: view.code,
    start: view.start,
    end: view.end,
    detail: view.detail,
  );

  static SnippetDetail _detail(api.SnippetDetailView view) => SnippetDetail(
    summary: _summary(view.snippet),
    bodyText: view.bodyText,
    bodyRich: view.bodyRich,
    revision: view.revision,
    fields: [
      for (final field in view.fields)
        TemplateField(
          name: field.name,
          label: field.label,
          kind: switch (field.kind) {
            api.TemplateFieldKind.text => TemplateFieldKind.text,
            api.TemplateFieldKind.multiline => TemplateFieldKind.multiline,
            api.TemplateFieldKind.choice => TemplateFieldKind.choice,
          },
          defaultValue: field.defaultValue,
          options: field.options,
          required: field.required_,
        ),
    ],
    lints: view.lints.map(_lint).toList(growable: false),
  );

  static SnippetCollection _collection(api.SnippetCollectionView view) =>
      SnippetCollection(
        id: view.id,
        name: view.name,
        displayName: view.displayName,
        icon:
            SnippetCollectionIcon.values
                .where((icon) => icon.name == view.icon)
                .firstOrNull ??
            SnippetCollectionIcon.folder,
        accent:
            SnippetCollectionAccent.values
                .where((accent) => accent.name == view.accent)
                .firstOrNull ??
            SnippetCollectionAccent.neutral,
        snippetCount: view.snippetCount,
      );

  static SnippetPreferences _preferences(api.SnippetPreferencesView view) =>
      SnippetPreferences(
        recordPasted: view.recordPasted,
        requireAuth: view.requireAuth,
      );
}
