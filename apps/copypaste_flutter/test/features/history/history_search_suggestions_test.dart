import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_search_suggestions.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/repository/recent_searches_store.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

const _facets = HistoryFacets(
  tags: [HistoryTag(id: 'tag-slides', name: 'Slides')],
  sourceApps: [HistorySourceAppFacet(id: 'app-slack', label: 'Slack')],
  originDevices: [
    HistoryDeviceFacet(
      id: 'device-studio',
      label: 'Studio Mac',
      deviceClass: DeviceClass.desktop,
    ),
  ],
);

void main() {
  test(
    'suggests matching facets and recents, then completes a filter',
    () async {
      final store = MemoryRecentSearchesStore();
      await store.write(['slack invoice', 'report']);
      final suggestions = HistorySearchSuggestionsController(store);
      addTearDown(suggestions.dispose);
      await suggestions.load();

      suggestions.update('sl', const HistoryQuery(), _facets);
      expect(suggestions.suggestions.map((suggestion) => suggestion.kind), [
        HistorySearchSuggestionKind.recent,
        HistorySearchSuggestionKind.tag,
        HistorySearchSuggestionKind.sourceApp,
      ]);

      // Active filters are not offered again; "mac" matches a later word.
      suggestions.update(
        'notes mac',
        const HistoryQuery(sourceApp: 'app-slack'),
        _facets,
      );
      expect(suggestions.suggestions.single.id, 'device-studio');

      suggestions.move(1);
      final completion = suggestions.complete(
        suggestions.highlightedSuggestion!,
        'notes mac',
        const HistoryQuery(search: 'notes mac', sourceApp: 'app-slack'),
      );
      expect(completion.search, 'notes');
      expect(completion.query.search, 'notes');
      expect(completion.query.origin, 'device-studio');
      expect(completion.query.sourceApp, 'app-slack');
      expect(suggestions.suggestions, isEmpty);
    },
  );

  test('recent searches are deduplicated, bounded and clearable', () async {
    final store = MemoryRecentSearchesStore();
    final suggestions = HistorySearchSuggestionsController(store);
    addTearDown(suggestions.dispose);

    for (var i = 0; i < 12; i++) {
      await suggestions.remember('query $i');
    }
    await suggestions.remember('  QUERY 5 ');
    expect(
      suggestions.recentSearches,
      hasLength(HistorySearchSuggestionsController.maxRecentSearches),
    );
    expect(suggestions.recentSearches.first, 'QUERY 5');
    expect(
      suggestions.recentSearches.where((entry) => entry.endsWith('5')),
      hasLength(1),
    );
    expect(await store.read(), suggestions.recentSearches);

    await suggestions.clearRecent();
    expect(suggestions.recentSearches, isEmpty);
    expect(await store.read(), isEmpty);
  });

  testWidgets('keyboard and taps complete suggestions into filters', (
    tester,
  ) async {
    final controller = HistoryController(
      _Repository(),
      searchDebounce: Duration.zero,
    );
    addTearDown(controller.dispose);
    await tester.binding.setSurfaceSize(const Size(1200, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await controller.initialize();
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    final search = find.byType(EditableText).first;
    await tester.tap(search);
    await tester.enterText(search, 'chat sl');
    await tester.pumpAndSettle();
    expect(find.text('Slides'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(controller.window.query.sourceApp, 'app-slack');
    expect(controller.window.query.search, 'chat');
    expect(tester.widget<EditableText>(search).controller.text, 'chat');
    expect(find.text('Slides'), findsNothing);

    await tester.enterText(search, 'chat sli');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Slides'));
    await tester.pumpAndSettle();
    expect(controller.window.query.tagIds, ['tag-slides']);
    expect(tester.takeException(), isNull);
  });
}

class _Repository implements HistoryRepository {
  @override
  Stream<HistoryRuntimeEvent> watch() =>
      Stream<HistoryRuntimeEvent>.multi((_) {});

  @override
  Future<HistoryFacets> facets() async => _facets;

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async => const HistoryClipPage(items: []);

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async => null;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
