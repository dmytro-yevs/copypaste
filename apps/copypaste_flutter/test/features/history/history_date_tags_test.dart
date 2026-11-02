import 'dart:async';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_date_filter.dart';
import 'package:copypaste_flutter/features/history/view/history_tags_dialog.dart';
import 'package:copypaste_flutter/features/history/view/history_tags_filter.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets(
    'compact date calendar fits the drawer width and available height',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(400, 900);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final repository = _Repository();
      addTearDown(repository.events.close);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await tester.pumpWidget(_app(controller));
      await tester.tap(find.byKey(const ValueKey('history-date-filter')));
      await tester.pumpAndSettle();
      final drawer = find.ancestor(
        of: find.byType(DatePickerDialog),
        matching: find.byType(DrawerWrapper),
      );
      expect(drawer, findsOneWidget);
      expect(
        tester.widget<DrawerWrapper>(drawer).position,
        OverlayPosition.bottom,
      );
      expect(tester.getRect(drawer).left, closeTo(0, 1));
      expect(tester.getRect(drawer).right, closeTo(400, 1));
      expect(tester.getRect(drawer).bottom, closeTo(900, 1));
      final apply = find.widgetWithText(Button, 'Apply');
      final calendar = find.byType(DatePickerDialog);
      final availableCalendar = find.ancestor(
        of: calendar,
        matching: find.byType(FittedBox),
      );
      final tallCalendar = tester.getRect(calendar);
      expect(
        tallCalendar.width,
        closeTo(tester.getRect(availableCalendar).width, 0.01),
      );
      tester.view.physicalSize = const Size(400, 500);
      await tester.pumpAndSettle();
      final shortCalendar = tester.getRect(calendar);
      expect(shortCalendar.height, lessThan(tallCalendar.height));
      expect(shortCalendar.bottom, lessThan(tester.getRect(apply).top));
      expect(tester.getRect(apply).bottom, lessThanOrEqualTo(500));
      tester.view.physicalSize = const Size(320, 900);
      await tester.pumpAndSettle();
      expect(
        tester.getRect(calendar).width,
        closeTo(tester.getRect(availableCalendar).width, 0.01),
      );
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      expect(find.byType(DatePickerDialog), findsNothing);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
    }),
  );

  testWidgets(
    'Windows date popover preserves other filters and clears the period',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1200, 900);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final repository = _Repository();
      addTearDown(repository.events.close);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.updateQuery(
        const HistoryQuery(
          search: 'needle',
          sourceApp: 'editor',
          tagIds: ['work'],
        ),
      );
      await tester.pumpWidget(_app(controller));
      await tester.tap(find.byKey(const ValueKey('history-date-filter')));
      await tester.pumpAndSettle();
      expect(find.byType(DatePickerDialog), findsOneWidget);
      expect(find.byType(AlertDialog), findsNothing);
      expect(find.textContaining('Choose one day'), findsNothing);
      final now = DateTime.now();
      await tester.tap(find.text('4').first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('19').first);
      await tester.pumpAndSettle();
      final selected = tester
          .widget<Calendar>(find.byType(Calendar))
          .value!
          .toRange();
      expect(selected.start, DateTime(now.year, now.month, 4));
      expect(selected.end, DateTime(now.year, now.month, 19));
      await tester.tap(find.text('Apply'));
      await tester.pumpAndSettle();
      expect(
        repository.queryValue.capturedFrom,
        DateTime(now.year, now.month, 4),
      );
      expect(
        repository.queryValue.capturedBefore,
        DateTime(now.year, now.month, 20),
      );
      await tester.tap(find.byKey(const ValueKey('history-date-filter')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Last 7 days'));
      await tester.pumpAndSettle();
      expect(
        repository.queryValue.capturedFrom,
        DateTime(now.year, now.month, now.day - 6),
      );
      expect(
        repository.queryValue.capturedBefore,
        DateTime(now.year, now.month, now.day + 1),
      );
      expect(repository.queryValue.search, 'needle');
      expect(repository.queryValue.tagIds, ['work']);
      expect(repository.queryValue.sourceApp, 'editor');
      expect(find.byType(Chip), findsNothing);
      expect(
        find.descendant(
          of: find.byType(HistoryDateFilter),
          matching: find.byType(AvatarBadge),
        ),
        findsOneWidget,
      );
      await tester.tap(find.byKey(const ValueKey('history-date-filter')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('All time'));
      await tester.pumpAndSettle();
      expect(
        find.descendant(
          of: find.byType(HistoryDateFilter),
          matching: find.byType(AvatarBadge),
        ),
        findsNothing,
      );
      expect(repository.queryValue.hasDateFilter, isFalse);
      expect(repository.queryValue.tagIds, ['work']);
      await tester.tap(find.byKey(const ValueKey('history-tags-filter')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Manage tags'));
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsNothing);
      expect(find.text('Tags'), findsOneWidget);
      expect(find.byType(TextField), findsOneWidget);
      expect(find.text('Done'), findsNothing);
      expect(find.text('Cancel'), findsNothing);
      expect(find.text('Save'), findsNothing);
      await tester.enterText(find.byType(TextField), 'Research');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();
      expect(repository.catalogue.map((tag) => tag.name), contains('Research'));
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller?.text,
        isEmpty,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({TargetPlatform.windows}),
  );

  testWidgets(
    'Android tags support mixed bulk assignment, rename and filtering',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(400, 850);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final repository = _Repository();
      addTearDown(repository.events.close);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.setDateRange(
        DateTime(2026, 10, 25),
        DateTime(2026, 10, 25),
      );
      expect(controller.query.capturedBefore, DateTime(2026, 10, 26));
      await tester.pumpWidget(_app(controller));
      await tester.tap(find.text('Tag both'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<Checkbox>(find.byType(Checkbox)).state,
        CheckboxState.indeterminate,
      );
      await tester.tap(find.byType(Checkbox));
      await tester.pumpAndSettle();
      expect(repository.assignments['b'], ['work']);
      expect(
        tester.widget<Checkbox>(find.byType(Checkbox)).state,
        CheckboxState.checked,
      );
      await tester.tap(find.byKey(const ValueKey('history-tag-actions-work')));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey('history-tag-name')),
        findsOneWidget,
        reason: 'The tag editor stays mounted beneath its action menu.',
      );
      await tester.tap(find.text('Rename'));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey('history-tag-name')),
        findsOneWidget,
        reason: 'Rename returns to the tag input.',
      );
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('history-tag-name')))
            .enabled,
        isTrue,
      );
      await tester.enterText(
        find.byKey(const ValueKey('history-tag-name')),
        'Projects',
      );
      await tester.tap(find.byKey(const ValueKey('history-tag-submit')));
      await tester.pumpAndSettle();
      expect(repository.catalogue.single.name, 'Projects');
      expect(find.text('Done'), findsNothing);
      expect(find.text('Cancel'), findsNothing);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('history-tags-filter')));
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsNothing);
      expect(find.byType(SelectPopup<String>), findsOneWidget);
      await tester.tap(find.text('Projects'));
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(repository.queryValue.tagIds, ['work']);
      expect(repository.queryValue.capturedFrom, DateTime(2026, 10, 25));
      expect(
        find.descendant(
          of: find.byType(HistoryTagsFilter),
          matching: find.byType(AvatarBadge),
        ),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({TargetPlatform.android}),
  );
}

Widget _app(HistoryController controller) => ShadcnApp(
  theme: AppTheme.light,
  builder: AppTheme.builder,
  home: Scaffold(
    child: AnimatedBuilder(
      animation: controller,
      builder: (context, _) => Column(
        children: [
          Row(
            children: [
              HistoryDateFilter(controller: controller),
              HistoryTagsFilter(controller: controller),
            ],
          ),
          Button.primary(
            onPressed: () => showHistoryTagsDialog(
              context,
              controller: controller,
              clipIds: ['a', 'b'],
            ),
            child: const Text('Tag both'),
          ),
        ],
      ),
    ),
  ),
);

class _Repository extends Fake
    implements HistoryRepository, HistoryTagsRepository {
  final events = StreamController<HistoryRuntimeEvent>.broadcast();
  HistoryQuery queryValue = const HistoryQuery();
  List<HistoryTag> catalogue = const [HistoryTag(id: 'work', name: 'Work')];
  final assignments = <String, List<String>>{
    'a': ['work'],
    'b': [],
  };
  @override
  Stream<HistoryRuntimeEvent> watch() => events.stream;
  @override
  Future<HistoryFacets> facets() async => HistoryFacets(tags: catalogue);
  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    queryValue = query;
    return const HistoryClipPage(items: []);
  }

  @override
  Future<HistoryTags> tags(List<String> clipIds) async => HistoryTags(
    tags: catalogue,
    assignments: {for (final id in clipIds) id: List.of(assignments[id] ?? [])},
  );
  @override
  Future<void> saveTag({String? id, required String name}) async {
    catalogue = [
      for (final tag in catalogue)
        if (tag.id != id) tag,
      HistoryTag(id: id ?? 'new', name: name),
    ];
  }

  @override
  Future<void> deleteTag(String id) async {
    catalogue = catalogue.where((tag) => tag.id != id).toList();
  }

  @override
  Future<void> changeTags({
    required List<String> clipIds,
    required List<String> tagIds,
    required bool assigned,
  }) async {
    for (final id in clipIds) {
      final next = (assignments[id] ?? []).toSet();
      assigned ? next.addAll(tagIds) : next.removeAll(tagIds);
      assignments[id] = next.toList();
    }
  }
}
