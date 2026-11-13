import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/content_rules/models/content_rule_models.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/history_expiry_text.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/repository/runtime_history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_clip_menu.dart';
import 'package:copypaste_flutter/features/history/view/history_inspector.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:copypaste_flutter/features/snippets/repository/clip_edit_preferences.dart';
import 'package:copypaste_flutter/features/snippets/repository/snippet_repository.dart';
import 'package:copypaste_flutter/features/transforms/repository/transform_recents_store.dart';
import 'package:copypaste_flutter/features/transforms/repository/transform_repository.dart';
import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/generated/api/clips.dart' as clip_api;
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  final now = DateTime.now();

  HistoryClip textClip(String id, {HistoryClipAnnotations? annotations}) =>
      HistoryClip(
        id: id,
        contentType: 'text/plain',
        preview: 'Clip $id',
        createdAt: now,
        pinned: false,
        sourceApp: 'Editor',
        annotations: annotations ?? HistoryClipAnnotations.none,
      );

  Future<HistoryController> pumpScreen(
    WidgetTester tester,
    _FeatureRepository repository, {
    Size size = const Size(1200, 800),
  }) async {
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.binding.setSurfaceSize(size);
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await _settle(tester);
    return controller;
  }

  testWidgets('the inspector and the row menu offer the same actions', (
    tester,
  ) async {
    final clip = textClip(
      'a',
      annotations: HistoryClipAnnotations(
        expiresAt: now.add(const Duration(minutes: 5)),
      ),
    );
    final repository = _FeatureRepository([clip]);
    final controller = await pumpScreen(tester, repository);
    await controller.selection.select('a');
    await _settle(tester);

    final rowMenu = <String>{};
    final rowContext = tester.element(
      find.byKey(const ValueKey<String>('history-clip-a')),
    );
    for (final item in historyClipMenuItems(
      rowContext,
      controller: controller,
      clip: clip,
    )) {
      final key = item.key;
      if (key is ValueKey<String>) rowMenu.add(key.value);
    }

    final inspector = <HistoryClipAction>{
      if (find.byKey(const ValueKey('hide-from-ai')).evaluate().isNotEmpty)
        HistoryClipAction.hideFromAi,
      if (find
          .byKey(const ValueKey('history-detail-action-transform'))
          .evaluate()
          .isNotEmpty)
        HistoryClipAction.transform,
    };
    await tester.tap(find.byKey(const ValueKey('history-detail-more')));
    await _settle(tester);
    for (final action in HistoryClipAction.values) {
      if (find
          .byKey(ValueKey('history-detail-action-${action.id}-a'))
          .evaluate()
          .isNotEmpty) {
        inspector.add(action);
      }
    }

    expect(inspector, HistoryClipAction.values.toSet());
    expect(rowMenu, {
      for (final action in HistoryClipAction.values)
        'history-menu-${action.id}-a',
      'history-menu-copy-plain-a',
      'history-menu-secret-a',
      'history-menu-delete-a',
    });
  });

  testWidgets('actions need their capability and a suitable clip', (
    tester,
  ) async {
    final repository = _FeatureRepository([textClip('a')]);
    final controller = await pumpScreen(tester, repository);
    final image = HistoryClip(
      id: 'image',
      contentType: 'image/png',
      preview: '',
      createdAt: now,
      pinned: false,
      kind: HistoryClipKind.image,
    );
    expect(historyClipActions(controller, image), [
      HistoryClipAction.addToQueue,
      HistoryClipAction.hideFromAi,
      HistoryClipAction.explain,
    ]);
    // Keep appears only while a deletion is scheduled.
    expect(
      historyClipActions(controller, textClip('a')),
      isNot(contains(HistoryClipAction.keep)),
    );

    final plain = HistoryController(_PlainRepository());
    addTearDown(plain.dispose);
    expect(historyClipActions(plain, textClip('a')), isEmpty);
  });

  testWidgets('the drawer footer stays fixed while the clip body scrolls', (
    tester,
  ) async {
    final repository = _FeatureRepository([
      HistoryClip(
        id: 'long',
        contentType: 'text/plain',
        preview: 'Long clip',
        body: List<String>.filled(80, 'Scrollable detail').join('\n'),
        createdAt: now,
        pinned: false,
      ),
    ]);
    await pumpScreen(tester, repository, size: const Size(480, 800));
    await tester.tap(find.text('Long clip'));
    await _settle(tester);

    final scroll = find.byKey(
      const ValueKey<String>('history-detail-scroll-content'),
    );
    final footer = find.byKey(const ValueKey<String>('history-detail-actions'));
    expect(find.descendant(of: scroll, matching: footer), findsNothing);
    final more = find.byKey(const ValueKey('history-detail-more'));
    expect(find.descendant(of: footer, matching: more), findsOneWidget);
    final top = tester.getTopLeft(footer).dy;
    await tester.drag(scroll, const Offset(0, -300));
    await tester.pump();
    expect(tester.getTopLeft(footer).dy, top);
    expect(more.hitTestable(), findsOneWidget);
  });

  testWidgets('labels collapse to icons once capability icons do not fit', (
    tester,
  ) async {
    // An image clip shows exactly the budgeted actions.
    final image = HistoryClip(
      id: 'image',
      contentType: 'image/png',
      preview: '',
      createdAt: now,
      pinned: false,
      kind: HistoryClipKind.image,
    );
    Future<double> threshold(HistoryRepository repository) async {
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.selection.select('image');
      Future<bool> labeled(double width) async {
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            home: Scaffold(
              child: Align(
                alignment: Alignment.topLeft,
                child: SizedBox(
                  width: width,
                  height: 600,
                  child: HistoryInspector(
                    controller: controller,
                    inDrawer: true,
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pump();
        return find
            .descendant(
              of: find.byKey(const ValueKey('history-detail-delete')),
              matching: find.text('Delete'),
            )
            .evaluate()
            .isNotEmpty;
      }

      var low = 200.0;
      var high = 1200.0;
      while (high - low > 1) {
        final middle = (low + high) / 2;
        if (await labeled(middle)) {
          high = middle;
        } else {
          low = middle;
        }
      }
      await labeled(high);
      return high;
    }

    final plain = await threshold(_PlainRepository([image]));
    final featured = await threshold(_FeatureRepository([image]));
    // The two capability icons (AI tools and More) join the label budget.
    expect(featured - plain, greaterThan(2 * AppControlSize.compact));
    expect(find.byKey(const ValueKey('hide-from-ai')), findsOneWidget);
    expect(find.byKey(const ValueKey('history-detail-more')), findsOneWidget);
  });

  testWidgets('hiding from AI tools and keeping update the selected clip', (
    tester,
  ) async {
    final repository = _FeatureRepository([
      textClip(
        'a',
        annotations: HistoryClipAnnotations(
          expiresAt: now.add(const Duration(minutes: 5)),
        ),
      ),
    ]);
    final controller = await pumpScreen(tester, repository);
    await controller.selection.select('a');
    await _settle(tester);
    final l10n = AppStrings.current;
    expect(find.text(l10n.historyDeletesInMinutes(5)), findsWidgets);

    await tester.tap(find.byKey(const ValueKey('hide-from-ai')));
    await _settle(tester);
    expect(repository.hidden, hasLength(1));
    expect(repository.hidden.single.$1, ['a']);
    expect(repository.hidden.single.$2, isTrue);
    expect(controller.selection.selectedClip!.annotations.hiddenFromAi, isTrue);
    expect(find.bySemanticsLabel(l10n.aiToolsShow), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('history-detail-more')));
    await _settle(tester);
    await tester.tap(
      find.byKey(const ValueKey('history-detail-action-keep-a')),
    );
    await _settle(tester);
    expect(repository.kept, ['a']);
    expect(find.text(l10n.historyDeletesInMinutes(5)), findsNothing);
    await tester.pump(const Duration(seconds: 6));
  });

  testWidgets('rows mark provenance, rule tags and the deletion countdown', (
    tester,
  ) async {
    final repository = _FeatureRepository([
      textClip(
        'a',
        annotations: HistoryClipAnnotations(
          provenance: HistoryProvenance.edited,
          ruleTagIds: const ['t-code'],
          expiresAt: now.add(const Duration(minutes: 90)),
        ),
      ),
    ]);
    await pumpScreen(tester, repository);
    final meta = tester.widget<RichText>(
      find
          .descendant(
            of: find.byKey(const ValueKey('history-clip-meta-a')),
            matching: find.byType(RichText),
          )
          .first,
    );
    final text = meta.text.toPlainText();
    final l10n = AppStrings.current;
    expect(text, contains('(${l10n.historyProvenanceEdited})'));
    expect(text, contains(l10n.historyRuleTag('code')));
    expect(find.text(l10n.historyDeletesInHours(2)), findsOneWidget);
  });

  testWidgets('bulk Add to Paste Queue keeps the selection order', (
    tester,
  ) async {
    final repository = _FeatureRepository([
      textClip('x'),
      textClip('y'),
      textClip('z'),
    ]);
    final controller = await pumpScreen(tester, repository);
    controller.bulk.beginBulkSelection('x');
    controller.bulk.toggleBulkSelection('z');
    controller.bulk.toggleBulkSelection('y');
    await _settle(tester);
    await tester.tap(find.byKey(const ValueKey('history-bulk-queue')));
    await _settle(tester);
    expect(repository.queued, [
      ['x', 'z', 'y'],
    ]);
    expect(
      find.text(AppStrings.current.historyQueueAddedInOrder(3)),
      findsOneWidget,
    );

    // A range is added top to bottom as displayed.
    controller.bulk.endBulkSelection();
    controller.bulk.beginBulkSelection('z');
    controller.bulk.selectBulkClip(
      'x',
      orderedIds: const ['x', 'y', 'z'],
      range: true,
    );
    await _settle(tester);
    await tester.tap(find.byKey(const ValueKey('history-bulk-queue')));
    await _settle(tester);
    expect(repository.queued.last, ['x', 'y', 'z']);
    // Let the confirmation toasts expire.
    await tester.pump(const Duration(seconds: 6));
  });

  test('maps runtime annotations onto History clips', () {
    final clip = RuntimeHistoryRepository.mapListClip(
      runtime.Clip(
        secret: false,
        transient: false,
        id: 'a',
        content: 'text',
        contentType: 'text/plain',
        contentClass: runtime.ClipContentClass.text,
        createdAtMs: 0,
        pinned: false,
        tagIds: const [],
        originDeviceClass: runtime.DeviceClass.unknown,
        truncated: false,
        tooLargeToSync: false,
        annotations: clip_api.ClipAnnotationState(
          expiresAtMs: 60000,
          hiddenFromAi: true,
          ruleSecret: true,
          provenance: const clip_api.ClipProvenance(
            kind: clip_api.ClipProvenanceKind.aiTool,
            transformSnapshot: [],
          ),
          ruleTagIds: const ['t'],
          keep: false,
        ),
      ),
    );
    expect(clip.annotations.expiresAt, DateTime.utc(1970, 1, 1, 0, 1));
    expect(clip.annotations.provenance, HistoryProvenance.aiTool);
    expect(clip.annotations.ruleTagIds, ['t']);
    expect(clip.masked, isTrue);
    expect(clip.secret, isFalse);
  });

  test('the countdown rounds up and changes only at unit boundaries', () {
    final l10n = AppStrings.current;
    final (minutes, untilFour) = HistoryExpiryText.describe(
      l10n,
      const Duration(minutes: 4, seconds: 30),
    );
    expect(minutes, l10n.historyDeletesInMinutes(5));
    expect(untilFour, const Duration(seconds: 30));
    expect(
      HistoryExpiryText.describe(l10n, const Duration(hours: 3)).$1,
      l10n.historyDeletesInHours(3),
    );
    expect(
      HistoryExpiryText.describe(l10n, const Duration(days: 3)).$1,
      l10n.historyDeletesInDays(3),
    );
    expect(HistoryExpiryText.describe(l10n, Duration.zero), (
      l10n.historyDeletesSoon,
      null,
    ));
  });
}

Future<void> _settle(WidgetTester tester) async {
  // Tooltip anchors keep a ticker alive, so pumpAndSettle cannot finish.
  await tester.pump();
  await tester.pump(const Duration(seconds: 1));
  await tester.pump(AppMotion.standard);
}

class _PlainRepository implements HistoryRepository {
  _PlainRepository([List<HistoryClip> clips = const []])
    : clips = List.of(clips);

  final List<HistoryClip> clips;

  @override
  Stream<HistoryRuntimeEvent> watch() =>
      Stream<HistoryRuntimeEvent>.multi((_) {});

  @override
  Future<HistoryFacets> facets() async => const HistoryFacets(
    tags: [HistoryTag(id: 't-code', name: 'code')],
  );

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async => HistoryClipPage(items: List.of(clips));

  @override
  Future<HistoryClip> get(String id) async =>
      clips.firstWhere((clip) => clip.id == id);

  @override
  Future<HistoryImagePreview?> imagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) async => null;

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async => null;

  @override
  Future<void> copy(String id) async {}

  @override
  Future<void> copyPlainText(String id) async {}

  @override
  Future<void> copyText(String id, String text) async {}

  @override
  Future<void> saveFile(String id, String destinationPath) async {}

  @override
  Future<void> importFile(HistoryImportFile file) async {}

  @override
  Future<HistoryClip> setSecret(String id, bool secret) =>
      throw UnimplementedError();

  @override
  Future<void> setPinned(String id, bool pinned) async {}

  @override
  Future<void> delete(String id) async {}

  @override
  Future<void> deleteAll() async {}

  @override
  Future<void> reorderPinned(List<String> ids) async {}
}

class _FeatureRepository extends _PlainRepository
    implements
        HistoryEditRepository,
        HistoryQueueRepository,
        HistoryTransformRepository,
        HistoryAnnotationsRepository {
  _FeatureRepository(super.clips);

  final hidden = <(List<String>, bool)>[];
  final kept = <String>[];
  final queued = <List<String>>[];

  @override
  ClipEditRepository get clipEdits => _UnusedEdits();

  @override
  SnippetRepository get snippets => _UnusedEdits();

  @override
  ClipEditPreferences get clipEditPreferences => MemoryClipEditPreferences();

  @override
  TransformRepository get transforms => _UnusedTransforms();

  @override
  TransformRecentsStore get transformRecents => MemoryTransformRecentsStore();

  @override
  Future<int> addToQueue(List<String> clipIds) async {
    queued.add(List.of(clipIds));
    return clipIds.length;
  }

  void _update(
    String id,
    HistoryClipAnnotations Function(HistoryClipAnnotations) change,
  ) {
    final index = clips.indexWhere((clip) => clip.id == id);
    clips[index] = clips[index].copyWith(
      annotations: change(clips[index].annotations),
    );
  }

  @override
  Future<void> setHiddenFromAi(
    List<String> clipIds, {
    required bool hidden,
  }) async {
    this.hidden.add((clipIds, hidden));
    for (final id in clipIds) {
      _update(
        id,
        (current) => HistoryClipAnnotations(
          expiresAt: current.expiresAt,
          hiddenFromAi: hidden,
        ),
      );
    }
  }

  @override
  Future<HistoryClipAnnotations> keep(String clipId) async {
    kept.add(clipId);
    _update(
      clipId,
      (current) => HistoryClipAnnotations(
        hiddenFromAi: current.hiddenFromAi,
        keep: true,
      ),
    );
    return (await get(clipId)).annotations;
  }

  @override
  Future<RuleTrace> ruleTrace(String clipId) async => const RuleTrace();

  @override
  Future<RuleOptions> ruleOptions() async => const RuleOptions();
}

/// Ports the tests never reach; any call fails the test.
class _UnusedEdits implements ClipEditRepository, SnippetRepository {
  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError(invocation.memberName.toString());
}

class _UnusedTransforms implements TransformRepository {
  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError(invocation.memberName.toString());
}
