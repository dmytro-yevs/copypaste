import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:flutter/foundation.dart';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets(
    'collapses each history section independently from its full header',
    (tester) async {
      final mobile = defaultTargetPlatform == TargetPlatform.android;
      await tester.binding.setSurfaceSize(Size(mobile ? 480 : 1400, 1000));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final now = DateTime.now();
      final older = DateTime(now.year, now.month, now.day - 3, 12);
      final repository = _PinRepository([
        _clip('Pinned clip', older, pinned: true),
        _clip('Today clip', now),
        _clip('Yesterday clip', DateTime(now.year, now.month, now.day - 1, 12)),
        _clip('Older clip', older),
      ]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          darkTheme: AppTheme.dark,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();

      final sections = {
        'history-section-pinned': 'Pinned clip',
        'history-section-today': 'Today clip',
        'history-section-yesterday': 'Yesterday clip',
        _dateSectionKey(older): 'Older clip',
      };
      final hidden = <String>{};
      for (final section in sections.entries) {
        final header = find.byKey(ValueKey<String>(section.key));
        expect(find.text(section.value), findsOneWidget);
        // Clicking the divider line also activates the section button.
        final rect = tester.getRect(header);
        await tester.tapAt(Offset(rect.left + 20, rect.center.dy));
        await tester.pumpAndSettle();
        hidden.add(section.value);
        expect(header, findsOneWidget);
        for (final clip in sections.values) {
          expect(
            find.text(clip),
            hidden.contains(clip) ? findsNothing : findsOneWidget,
          );
        }
      }
      expect(controller.items, hasLength(4));
      expect(controller.selectedId, isNull);
      expect(find.text('No clips found'), findsNothing);
      expect(repository.pinnedUpdates, isEmpty);

      // Reloads and responsive layout changes must not reopen hidden sections.
      await controller.reload();
      await tester.binding.setSurfaceSize(Size(mobile ? 1400 : 480, 1000));
      await tester.pumpAndSettle();
      for (final clip in sections.values) {
        expect(find.text(clip), findsNothing);
      }
      for (final section in sections.entries) {
        await tester.tap(find.byKey(ValueKey<String>(section.key)));
        await tester.pumpAndSettle();
        expect(find.text(section.value), findsOneWidget);
      }
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.linux,
      TargetPlatform.android,
    }),
  );

  testWidgets(
    'can reach later pages after collapsing the only loaded section',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _PinRepository([
        _clip('Pinned clip', DateTime.now(), pinned: true),
        _clip('Today clip', DateTime.now()),
      ]);
      final controller = HistoryController(repository, pageSize: 1);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Pinned clip'), findsOneWidget);
      expect(find.text('Today clip'), findsNothing);

      await tester.tap(
        find.byKey(const ValueKey<String>('history-section-pinned')),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(Button, 'Load more'));
      await tester.pumpAndSettle();
      expect(find.text('Pinned clip'), findsNothing);
      expect(find.text('Today clip'), findsOneWidget);
      expect(find.widgetWithText(Button, 'Load more'), findsNothing);
      expect(controller.items, hasLength(2));
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('keeps the drawer pin state reactive and visibly selected', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(799, 900);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final clip = HistoryClip(
      id: 'drawer-pin',
      contentType: 'text/plain',
      preview: 'Drawer pin clip',
      createdAt: DateTime.utc(2026),
      pinned: false,
    );
    final repository = _PinRepository([clip]);
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Drawer pin clip'));
    await tester.pumpAndSettle();
    final pinButton = find.byKey(
      const ValueKey<String>('history-pin-drawer-pin'),
    );
    expect(find.widgetWithText(Button, 'Pin'), findsOneWidget);
    expect(
      (tester
                  .widget<Button>(pinButton)
                  .style
                  .decoration(tester.element(pinButton), const {})
              as BoxDecoration)
          .color,
      Theme.of(tester.element(pinButton)).colorScheme.input.scaleAlpha(0.3),
    );

    await tester.tap(pinButton);
    await tester.pumpAndSettle();

    expect(
      find.descendant(of: pinButton, matching: find.text('Pinned')),
      findsOneWidget,
    );
    expect(
      (tester
                  .widget<Button>(pinButton)
                  .style
                  .decoration(tester.element(pinButton), const {})
              as BoxDecoration)
          .color,
      Theme.of(tester.element(pinButton)).colorScheme.secondary,
    );
    expect(repository.pinnedUpdates, [('drawer-pin', true)]);

    await tester.tap(pinButton);
    await tester.pumpAndSettle();

    expect(find.widgetWithText(Button, 'Pin'), findsOneWidget);
    expect(repository.pinnedUpdates, [
      ('drawer-pin', true),
      ('drawer-pin', false),
    ]);
  });

  testWidgets('groups newest history into pinned and calendar sections', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final now = DateTime.now();
    final older = DateTime(now.year, now.month, now.day - 3, 12).toUtc();
    final repository = _PinRepository([
      _clip('Pinned clip', older, pinned: true),
      _clip('Older clip', older),
      _clip('Today clip', DateTime(now.year, now.month, now.day, 12).toUtc()),
      _clip(
        'Yesterday clip',
        DateTime(now.year, now.month, now.day - 1, 12).toUtc(),
      ),
    ]);
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    final pinned = find.byKey(const ValueKey<String>('history-section-pinned'));
    final today = find.byKey(const ValueKey<String>('history-section-today'));
    final yesterday = find.byKey(
      const ValueKey<String>('history-section-yesterday'),
    );
    final olderDate = find.byKey(ValueKey<String>(_dateSectionKey(older)));
    expect(pinned, findsOneWidget);
    expect(today, findsOneWidget);
    expect(yesterday, findsOneWidget);
    expect(olderDate, findsOneWidget);
    expect(tester.getTopLeft(pinned).dy, lessThan(tester.getTopLeft(today).dy));
    expect(
      tester.getTopLeft(today).dy,
      lessThan(tester.getTopLeft(yesterday).dy),
    );
    expect(
      tester.getTopLeft(yesterday).dy,
      lessThan(tester.getTopLeft(olderDate).dy),
    );
    expect(find.text('Pinned clip'), findsOneWidget);
  });

  testWidgets('reverses calendar sections for oldest sorting', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final now = DateTime.now();
    final older = DateTime(now.year, now.month, now.day - 3, 12).toUtc();
    final repository = _PinRepository([
      _clip('Pinned clip', older, pinned: true),
      _clip('Today clip', DateTime(now.year, now.month, now.day, 12).toUtc()),
      _clip(
        'Yesterday clip',
        DateTime(now.year, now.month, now.day - 1, 12).toUtc(),
      ),
      _clip('Older clip', older),
    ]);
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await controller.updateQuery(const HistoryQuery(sort: HistorySort.oldest));

    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    final pinned = find.byKey(const ValueKey<String>('history-section-pinned'));
    final olderDate = find.byKey(ValueKey<String>(_dateSectionKey(older)));
    final yesterday = find.byKey(
      const ValueKey<String>('history-section-yesterday'),
    );
    final today = find.byKey(const ValueKey<String>('history-section-today'));
    expect(
      tester.getTopLeft(pinned).dy,
      lessThan(tester.getTopLeft(olderDate).dy),
    );
    expect(
      tester.getTopLeft(olderDate).dy,
      lessThan(tester.getTopLeft(yesterday).dy),
    );
    expect(
      tester.getTopLeft(yesterday).dy,
      lessThan(tester.getTopLeft(today).dy),
    );
  });

  testWidgets('keeps relevance results flat below the pinned section', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final now = DateTime.now();
    final repository = _PinRepository([
      _clip('Pinned result', now, pinned: true),
      _clip('Today result', now),
      _clip('Older result', now.subtract(const Duration(days: 3))),
    ]);
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await controller.updateQuery(
      const HistoryQuery(search: 'result', sort: HistorySort.relevance),
    );

    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('history-section-pinned')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('history-section-today')),
      findsNothing,
    );
    expect(
      find.byKey(const ValueKey<String>('history-section-yesterday')),
      findsNothing,
    );
    expect(find.text('Pinned result'), findsOneWidget);
    expect(find.text('Today result'), findsOneWidget);
    expect(find.text('Older result'), findsOneWidget);

    await tester.tap(
      find.byKey(const ValueKey<String>('history-section-pinned')),
    );
    await tester.pumpAndSettle();
    expect(find.text('Pinned result'), findsNothing);
    expect(find.text('Today result'), findsOneWidget);
    expect(find.text('Older result'), findsOneWidget);
  });
}

class _PinRepository implements HistoryRepository {
  @override
  Future<void> importFile(HistoryImportFile file) async {}

  _PinRepository(this.items);

  final List<HistoryClip> items;
  final List<(String, bool)> pinnedUpdates = [];

  @override
  Stream<HistoryRuntimeEvent> watch() => const Stream.empty();

  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    final start = int.parse(cursor ?? '0');
    final end = (start + limit).clamp(0, items.length);
    return HistoryClipPage(
      items: items.sublist(start, end),
      nextCursor: end < items.length ? '$end' : null,
    );
  }

  @override
  Future<HistoryClip> get(String id) async =>
      items.firstWhere((item) => item.id == id);

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
  Future<void> saveFile(String id, String destinationPath) async {}

  @override
  Future<void> setPinned(String id, bool pinned) async {
    pinnedUpdates.add((id, pinned));
  }

  @override
  Future<void> delete(String id) async {}

  @override
  Future<void> deleteAll() async {}

  @override
  Future<void> reorderPinned(List<String> ids) async {}
}

HistoryClip _clip(String id, DateTime createdAt, {bool pinned = false}) =>
    HistoryClip(
      id: id,
      contentType: 'text/plain',
      preview: id,
      createdAt: createdAt,
      pinned: pinned,
    );

String _dateSectionKey(DateTime value) {
  final local = value.toLocal();
  final date = DateTime(local.year, local.month, local.day);
  return 'history-section-date-${date.toIso8601String().split('T').first}';
}
