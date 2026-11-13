import 'dart:async';
import 'dart:convert';

import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:flutter/foundation.dart' show defaultTargetPlatform;
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';

import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/app_shell.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/device_label.dart';
import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_window_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/history_clip_presentation.dart';
import 'package:copypaste_flutter/features/history/presentation/source_app_label.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_downloader.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:copypaste_flutter/features/snippets/repository/clip_edit_preferences.dart';
import 'package:copypaste_flutter/features/snippets/repository/snippet_repository.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as raster;
import '../../support/focus_traversal.dart';

void main() {
  testWidgets('focus rings fit the list and cards align with the toolbar', (
    tester,
  ) async {
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'ring',
            contentType: 'text',
            preview: 'Ring clip',
            createdAt: DateTime.now(),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
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

    final viewport = tester.getRect(find.byType(CustomScrollView));
    final band = tester.getRect(
      find.byKey(const ValueKey<String>('history-section-today')),
    );
    final card = tester.getRect(
      find.byKey(const ValueKey<String>('history-clip-ring')),
    );
    // FocusOutline paints its ring 3 px outside the focused control.
    for (final ring in [band.inflate(3), card.inflate(3)]) {
      expect(ring.left, greaterThanOrEqualTo(viewport.left));
      expect(ring.top, greaterThanOrEqualTo(viewport.top));
      expect(ring.right, lessThanOrEqualTo(viewport.right));
    }
    // Cards and bands keep the page column that the toolbar stretches across.
    final screen = tester.getRect(find.byType(HistoryScreen));
    expect(card.left, screen.left + AppSpacing.lg);
    expect(card.right, screen.right - AppSpacing.lg);
    expect(band.left, card.left);
    expect(band.right, card.right);
  });

  testWidgets('group bands name their exact size and never guess it', (
    tester,
  ) async {
    final now = DateTime.now();
    final today = DateTime(now.year, now.month, now.day);
    final repository = _CountingScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'pinned',
            contentType: 'text',
            preview: 'Pinned clip',
            createdAt: now,
            pinned: true,
          ),
          HistoryClip(
            id: 'today',
            contentType: 'text',
            preview: 'Today clip',
            createdAt: now,
            pinned: false,
          ),
        ],
      )
      ..counts = HistoryGroupCounts(pinned: 2, days: {today: 14});
    final controller = HistoryController(
      repository,
      includeFacets: false,
      includeGroupCounts: true,
    );
    addTearDown(controller.dispose);
    await tester.binding.setSurfaceSize(const Size(1200, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await controller.initialize();
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        localizationsDelegates: AppLocalization.delegates,
        supportedLocales: AppLocalization.supportedLocales,
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    final l10n = AppStrings.current;
    Finder count(String key) =>
        find.byKey(ValueKey<String>('history-section-count-$key'));
    expect(find.text(l10n.historySectionClipCount(14).toUpperCase()), findsOne);
    expect(find.text(l10n.historySectionClipCount(2).toUpperCase()), findsOne);
    // The count shares the band's line and is read with its name.
    expect(
      tester.getCenter(count('today')).dy,
      closeTo(
        tester.getCenter(find.text(l10n.historySectionToday.toUpperCase())).dy,
        1,
      ),
    );
    expect(
      find.bySemanticsLabel(
        RegExp(
          '^${l10n.historySectionPinned}, ${l10n.historySectionClipCount(2)}\$',
        ),
      ),
      findsOne,
    );

    // A search cannot be counted exactly, so no number is shown.
    repository.counts = null;
    controller.window.updateSearch('clip');
    await tester.pump(const Duration(seconds: 1));
    await tester.pumpAndSettle();
    expect(count('today'), findsNothing);
    expect(count('pinned'), findsNothing);
  });

  testWidgets('Tab moves on from search and Escape closes compact search', (
    tester,
  ) async {
    final controller = HistoryController(_TagScreenRepository());
    addTearDown(controller.dispose);
    addTearDown(() => tester.binding.setSurfaceSize(null));
    Future<void> pump(double width) async {
      await tester.binding.setSurfaceSize(Size(width, 800));
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();
    }

    await pump(1200);
    await expectTabLeaves(tester, find.byType(TextField));

    await pump(420);
    final toggle = find.byKey(const ValueKey<String>('history-search-toggle'));
    await tester.tap(toggle);
    await tester.pumpAndSettle();
    expect(find.byType(TextField), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byType(TextField), findsNothing);
    expect(toggle, findsOneWidget);
    // Clearing the query schedules the debounced search reload.
    await tester.pump(const Duration(seconds: 1));
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('icon filter actions show their tooltips on hover', (
    tester,
  ) async {
    final controller = HistoryController(_TagScreenRepository());
    addTearDown(controller.dispose);
    await tester.binding.setSurfaceSize(const Size(600, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);
    await mouse.moveTo(
      tester.getCenter(find.byIcon(LucideIcons.monitorSmartphone)),
    );
    await tester.pumpAndSettle();
    expect(find.text('All devices'), findsOneWidget);
    expect(
      find.byWidgetPredicate((widget) => widget is SelectPopup),
      findsNothing,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('retry reissues a request without changing selected filters', (
    tester,
  ) async {
    final repository = _RecoveryScreenRepository();
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.binding.setSurfaceSize(const Size(600, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();
    expect(controller.window.state, HistoryLoadState.error);
    expect(find.byKey(const ValueKey('history-date-filter')), findsOneWidget);
    final requests = repository.queries.length;
    repository.failAll = false;
    await tester.tap(find.text('Try again'));
    await tester.pumpAndSettle();
    expect(repository.queries.length, greaterThan(requests));
    expect(controller.window.state, HistoryLoadState.empty);
    repository.rejectFiltered = true;
    await controller.window.updateQuery(
      const HistoryQuery(kind: HistoryClipKind.image),
    );
    await tester.pumpAndSettle();
    expect(controller.window.state, HistoryLoadState.error);
    expect(find.byKey(const ValueKey('history-date-filter')), findsOneWidget);
    repository.rejectFiltered = false;
    final filteredRequests = repository.queries.length;
    await tester.tap(find.text('Try again'));
    await tester.pumpAndSettle();
    expect(repository.queries.length, greaterThan(filteredRequests));
    expect(controller.window.query.kind, HistoryClipKind.image);
    expect(controller.window.state, HistoryLoadState.empty);
    expect(find.byKey(const ValueKey('history-reset-filters')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('history-reset-filters')));
    await tester.pumpAndSettle();
    expect(controller.window.query.kind, isNull);
    expect(controller.window.hasActiveFilters, isFalse);
    expect(find.byKey(const ValueKey('history-reset-filters')), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('reuses code spans when the row and inspector rebuild', (
    tester,
  ) async {
    const code = 'fn main() { println!("hello"); }';
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'code-fixture',
            contentType: 'text',
            kind: HistoryClipKind.code,
            preview: code,
            body: code,
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await controller.initialize();
    await controller.selection.select('code-fixture');
    Widget app() => ShadcnApp(
      theme: AppTheme.light,
      builder: AppTheme.builder,
      home: Scaffold(child: HistoryScreen(controller: controller)),
    );
    await tester.pumpWidget(app());
    await tester.pumpAndSettle();
    final row = find.byWidgetPredicate(
      (widget) => widget is Text && widget.textSpan?.toPlainText() == code,
    );
    final inspector = find.byWidgetPredicate(
      (widget) =>
          widget is SelectableText && widget.textSpan?.toPlainText() == code,
    );
    expect(row, findsOneWidget);
    expect(inspector, findsOneWidget);
    final rowSpan = tester.widget<Text>(row).textSpan;
    final inspectorSpan = tester.widget<SelectableText>(inspector).textSpan;
    await tester.pumpWidget(app());
    await tester.pumpAndSettle();
    expect(identical(tester.widget<Text>(row).textSpan, rowSpan), isTrue);
    expect(
      identical(
        tester.widget<SelectableText>(inspector).textSpan,
        inspectorSpan,
      ),
      isTrue,
    );
  });

  for (final width in [390.0, 1400.0]) {
    testWidgets('Find opens and selects History search at $width', (
      tester,
    ) async {
      final controller = HistoryController(_ScreenRepository());
      final navigation = AppNavigationController();
      addTearDown(controller.dispose);
      addTearDown(navigation.dispose);
      await tester.binding.setSurfaceSize(Size(width, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(
            child: HistoryScreen(
              controller: controller,
              findRequests: navigation.historyFindRequests,
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      navigation.findInHistory();
      await tester.pumpAndSettle();
      final search = find.byType(EditableText).first;
      final editable = tester.widget<EditableText>(search);
      expect(editable.focusNode.hasFocus, isTrue);
      expect(navigation.historyFindRequests.value, isFalse);
      await tester.enterText(search, 'clipboard');
      await tester.pumpAndSettle();
      navigation.findInHistory();
      await tester.pumpAndSettle();
      expect(
        tester.widget<EditableText>(search).controller.selection,
        const TextSelection(baseOffset: 0, extentOffset: 9),
      );
      expect(tester.takeException(), isNull);
    });
  }
  testWidgets(
    'bulk taps select confidential clips without revealing their spoiler',
    (tester) async {
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'bulk-secret',
              contentType: 'text',
              preview: 'SYNTHETIC BULK SECRET',
              body: 'SYNTHETIC BULK SECRET',
              secret: true,
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pump();
      expect(
        find.semantics.byLabel(RegExp('Confidential content')),
        findsWidgets,
      );
      await tester.tap(find.byKey(const ValueKey('history-select-clips')));
      await tester.pump();
      final spoiler = find.byKey(const ValueKey('history-spoiler-bulk-secret'));
      expect(
        find.semantics.byLabel(RegExp('Confidential content')),
        findsNothing,
      );
      await tester.tapAt(tester.getCenter(spoiler));
      await tester.pump();
      expect(controller.bulk.bulkSelectedIds, {'bulk-secret'});
      expect(find.text('SYNTHETIC BULK SECRET').hitTestable(), findsNothing);
      expect(controller.selection.selectedId, isNull);
      await tester.tapAt(tester.getCenter(spoiler));
      await tester.pump();
      expect(controller.bulk.bulkSelectedIds, isEmpty);
      expect(find.text('SYNTHETIC BULK SECRET').hitTestable(), findsNothing);
      await tester.tap(find.byKey(const ValueKey('history-bulk-close')));
      await tester.pump();
      await tester.tapAt(tester.getCenter(spoiler));
      await tester.pump();
      await tester.pump(AppMotion.spoilerReveal);
      expect(find.text('SYNTHETIC BULK SECRET').hitTestable(), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'bulk button selects multiple clips, pins and unpins, and confirms deletion',
    (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      for (final width in [320.0, 1400.0]) {
        await tester.binding.setSurfaceSize(Size(width, 900));
        final repository = _ScreenRepository()
          ..page = HistoryClipPage(
            items: [
              for (final id in ['bulk-one', 'bulk-two', 'bulk-keep'])
                HistoryClip(
                  id: id,
                  contentType: 'text',
                  preview: id,
                  createdAt: DateTime.utc(2026),
                  pinned: id == 'bulk-one',
                ),
            ],
          );
        final controller = HistoryController(repository);
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            builder: AppTheme.builder,
            home: Scaffold(child: HistoryScreen(controller: controller)),
          ),
        );
        await _pumpHoverActions(tester);
        final entry = find.byKey(const ValueKey('history-select-clips'));
        final entrySize = tester.getSize(entry);
        expect(entrySize.width, entrySize.height);
        expect(
          entrySize.height,
          tester
              .getSize(find.byKey(const ValueKey('history-import-files')))
              .height,
        );
        await tester.tap(entry);
        await _pumpHoverActions(tester);
        expect(find.text('0 selected'), findsOneWidget);
        for (final id in ['bulk-one', 'bulk-two']) {
          await tester.tap(find.byKey(ValueKey('history-clip-$id')));
          await _pumpHoverActions(tester);
        }
        expect(find.text('2 selected'), findsOneWidget);
        expect(controller.selection.selectedId, isNull);
        expect(
          find.byKey(const ValueKey('history-detail-drawer')),
          findsNothing,
        );
        expect(find.byType(Checkbox), findsNWidgets(3));
        expect(
          find.byKey(const ValueKey('history-pin-handle-bulk-one')),
          findsNothing,
        );
        await tester.tap(find.byKey(const ValueKey('history-bulk-pin')));
        await _pumpHoverActions(tester);
        expect(repository.pinnedUpdates, [('bulk-two', true)]);
        await tester.tap(find.byKey(const ValueKey('history-bulk-unpin')));
        await _pumpHoverActions(tester);
        expect(repository.pinnedUpdates, [
          ('bulk-two', true),
          ('bulk-one', false),
          ('bulk-two', false),
        ]);
        await tester.tap(find.byKey(const ValueKey('history-bulk-delete')));
        await _pumpHoverActions(tester);
        expect(find.text('Delete 2 selected clips?'), findsOneWidget);
        expect(repository.deletedIds, isEmpty);
        await tester.tap(find.widgetWithText(Button, 'Cancel'));
        await _pumpHoverActions(tester);
        expect(controller.bulk.bulkSelectedIds, {'bulk-one', 'bulk-two'});
        await tester.tap(find.byKey(const ValueKey('history-bulk-delete')));
        await _pumpHoverActions(tester);
        await tester.tap(find.widgetWithText(Button, 'Delete'));
        await _pumpHoverActions(tester);
        expect(repository.deletedIds.toSet(), {'bulk-one', 'bulk-two'});
        expect(controller.window.items.single.id, 'bulk-keep');
        expect(controller.bulk.isBulkSelecting, isFalse);
        await tester.tap(find.byKey(const ValueKey('history-select-clips')));
        await _pumpHoverActions(tester);
        await tester.tap(find.byKey(const ValueKey('history-clip-bulk-keep')));
        await _pumpHoverActions(tester);
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await _pumpHoverActions(tester);
        expect(controller.bulk.isBulkSelecting, isFalse);
        expect(controller.bulk.bulkSelectedIds, isEmpty);
        await tester.pumpWidget(const SizedBox.shrink());
        controller.dispose();
        expect(tester.takeException(), isNull);
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'touch and mouse holds enter bulk selection with square controls at 200 percent text',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(320, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'hold',
              contentType: 'text',
              preview: 'Hold this clip',
              createdAt: DateTime.utc(2026),
              pinned: true,
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: (context, child) => AppTheme.builder(
            context,
            MediaQuery(
              data: MediaQuery.of(
                context,
              ).copyWith(textScaler: TextScaler.linear(2)),
              child: child!,
            ),
          ),
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await _pumpHoverActions(tester);
      for (final kind in [PointerDeviceKind.touch, PointerDeviceKind.mouse]) {
        await tester.longPress(
          find.byKey(const ValueKey('history-clip-hold')),
          kind: kind,
        );
        await _pumpHoverActions(tester);
        expect(controller.bulk.bulkSelectedIds, {'hold'});
        expect(controller.selection.selectedId, isNull);
        expect(find.text('1 selected'), findsOneWidget);
        final sizes = [
          for (final action in ['close', 'pin', 'unpin', 'delete'])
            tester.getSize(find.byKey(ValueKey('history-bulk-$action'))),
        ];
        expect(sizes.toSet(), hasLength(1));
        expect(sizes.first.width, sizes.first.height);
        expect(
          find.byKey(const ValueKey('history-row-pin-hold')),
          findsNothing,
        );
        expect(
          find.byKey(const ValueKey('history-row-more-hold')),
          findsNothing,
        );
        await tester.tap(find.byKey(const ValueKey('history-bulk-close')));
        await _pumpHoverActions(tester);
        expect(controller.bulk.isBulkSelecting, isFalse);
        expect(
          find.byKey(const ValueKey('history-select-clips')),
          findsOneWidget,
        );
        expect(tester.takeException(), isNull);
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'History and inspector share reveal, reselection and conceal without exposing hidden content',
    (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.binding.setSurfaceSize(const Size(1280, 800));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'secret-fixture',
              contentType: 'text',
              kind: HistoryClipKind.credentials,
              preview: 'SYNTHETIC HISTORY SECRET',
              body: 'SYNTHETIC HISTORY SECRET',
              secret: true,
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pump();
      expect(find.text('SYNTHETIC HISTORY SECRET').hitTestable(), findsNothing);
      await controller.selection.select('secret-fixture');
      await tester.pump();
      expect(find.text('Credentials'), findsWidgets);
      expect(find.text('SYNTHETIC HISTORY SECRET').hitTestable(), findsNothing);
      final inspectorSpoiler = find.byKey(
        const ValueKey('inspector-spoiler-secret-fixture-0'),
      );
      final inspectorBody = find
          .descendant(
            of: inspectorSpoiler,
            matching: find.byType(SelectableText),
          )
          .first;
      expect(
        tester.getSize(inspectorSpoiler).height,
        tester.getSize(inspectorBody).height,
      );
      expect(
        tester.getSize(inspectorSpoiler).width,
        tester.getSize(inspectorBody).width,
      );
      expect(
        tester.getSize(inspectorSpoiler).width,
        lessThan(
          tester
                  .getSize(
                    find.byKey(const ValueKey('history-detail-inspector')),
                  )
                  .width -
              AppSpacing.lg,
        ),
      );
      expect(tester.getSize(inspectorSpoiler).height, lessThan(100));
      final historySpoiler = find.byKey(
        const ValueKey('history-spoiler-secret-fixture'),
      );
      final historyBody = find
          .descendant(
            of: historySpoiler,
            matching: find.text('SYNTHETIC HISTORY SECRET'),
          )
          .first;
      expect(tester.getSize(historySpoiler), tester.getSize(historyBody));
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('inspector-spoiler-secret-fixture-0')),
          matching: find.byType(Button),
        ),
      );
      await tester.pump();
      await tester.pump(AppMotion.spoilerReveal);
      expect(
        find.text('SYNTHETIC HISTORY SECRET').hitTestable(),
        findsNWidgets(2),
      );
      controller.selection.clearSelection();
      await tester.pump();
      await controller.selection.select('secret-fixture');
      await tester.pump();
      await tester.pump(AppMotion.spoilerReveal);
      expect(
        find.text('SYNTHETIC HISTORY SECRET').hitTestable(),
        findsNWidgets(2),
      );
      await controller.spoilers.setRevealed('secret-fixture', false);
      await tester.pump();
      expect(find.text('SYNTHETIC HISTORY SECRET').hitTestable(), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets(
    'mobile history rows and errors clear the floating navigation',
    (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.binding.setSurfaceSize(const Size(320, 720));
      for (final textScale in [1.0, 2.0]) {
        final repository = _ScreenRepository()
          ..failPinUpdate = true
          ..page = HistoryClipPage(
            items: [
              for (var index = 0; index < 30; index++)
                HistoryClip(
                  id: 'clearance-$index',
                  contentType: 'text',
                  preview: 'Clearance clip $index',
                  createdAt: DateTime.utc(2026, 10, 8, 0, 0, 30 - index),
                  pinned: false,
                ),
            ],
          );
        final controller = HistoryController(repository);
        final navigation = AppNavigationController();
        addTearDown(controller.dispose);
        addTearDown(navigation.dispose);
        await controller.initialize();
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            builder: (context, child) => AppTheme.builder(
              context,
              MediaQuery(
                data: MediaQuery.of(context).copyWith(
                  padding: const EdgeInsets.only(bottom: 24),
                  viewPadding: const EdgeInsets.only(bottom: 24),
                  textScaler: TextScaler.linear(textScale),
                ),
                child: child!,
              ),
            ),
            home: AppShell(
              controller: navigation,
              destinations: {
                AppDestination.history: HistoryScreen(controller: controller),
                AppDestination.devices: const SizedBox.expand(),
                AppDestination.settings: const SizedBox.expand(),
              },
            ),
          ),
        );
        await tester.pumpAndSettle();
        final list = find.byType(CustomScrollView);
        await tester.drag(list, const Offset(0, -10000));
        await tester.pumpAndSettle();
        final position = navigation
            .scrollControllerFor(AppDestination.history)
            .position;
        position.jumpTo(position.maxScrollExtent);
        await tester.pumpAndSettle();
        final dockTop = tester
            .getRect(find.byKey(const ValueKey('mobile-navigation-dock')))
            .top;
        expect(
          tester.getRect(find.text('Clearance clip 29')).bottom,
          lessThan(dockTop),
        );
        await controller.mutations.togglePin(controller.window.items.last);
        await tester.pumpAndSettle();
        position.jumpTo(position.maxScrollExtent);
        await tester.pumpAndSettle();
        expect(tester.getRect(find.byType(Alert)).bottom, lessThan(dockTop));
        await tester.scrollUntilVisible(
          find.text('Clearance clip 29'),
          -200,
          scrollable: find
              .descendant(of: list, matching: find.byType(Scrollable))
              .first,
        );
        await tester.pumpAndSettle();
        expect(
          tester.getCenter(find.text('Clearance clip 29')).dy,
          lessThan(dockTop),
        );
        expect(find.text('Clearance clip 29').hitTestable(), findsOneWidget);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'overlays compact copy, pin and more actions on mouse hover',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'actions',
              contentType: 'text',
              preview: 'Hover action clip',
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
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
      await _pumpHoverActions(tester);
      final clip = find.byKey(const ValueKey<String>('history-clip-actions'));
      final copy = find.byKey(
        const ValueKey<String>('history-row-copy-actions'),
      );
      final pin = find.byKey(const ValueKey<String>('history-row-pin-actions'));
      final more = find.byKey(
        const ValueKey<String>('history-row-more-actions'),
      );
      final delete = find.byKey(
        const ValueKey<String>('history-menu-delete-actions'),
      );
      final rect = tester.getRect(clip);
      expect(copy, findsNothing);
      expect(pin, findsNothing);
      expect(more, findsNothing);

      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      addTearDown(mouse.removePointer);
      await mouse.moveTo(rect.center);
      await _pumpHoverActions(tester);
      expect(copy, findsOneWidget);
      expect(pin, findsOneWidget);
      expect(more, findsOneWidget);
      for (final action in [copy, pin, more]) {
        expect(tester.getSize(action), const Size.square(24));
      }
      expect(tester.getCenter(copy).dx, lessThan(tester.getCenter(pin).dx));
      expect(tester.getCenter(pin).dx, lessThan(tester.getCenter(more).dx));
      expect(tester.getRect(clip), rect);
      expect(rect.contains(tester.getCenter(more)), isTrue);

      await mouse.moveTo(Offset.zero);
      await _pumpHoverActions(tester);
      expect(pin, findsNothing);
      expect(more, findsNothing);
      await mouse.moveTo(rect.center);
      await _pumpHoverActions(tester);

      await mouse.moveTo(tester.getCenter(pin));
      await _pumpHoverActions(tester);
      expect(pin, findsOneWidget);
      await tester.tap(pin, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      expect(repository.pinnedUpdates, [('actions', true)]);
      expect(controller.selection.selectedId, isNull);
      await mouse.moveTo(tester.getCenter(clip));
      await _pumpHoverActions(tester);
      expect(find.byIcon(LucideIcons.pinOff), findsWidgets);
      await tester.tap(pin, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      expect(repository.pinnedUpdates, [('actions', true), ('actions', false)]);

      await mouse.moveTo(tester.getCenter(clip));
      await _pumpHoverActions(tester);
      await tester.tap(more, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      await tester.tap(delete, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      expect(find.text('Delete this clip?'), findsOneWidget);
      expect(repository.deletedIds, isEmpty);
      await tester.tap(
        find.widgetWithText(Button, 'Cancel'),
        kind: PointerDeviceKind.mouse,
      );
      await _pumpHoverActions(tester);
      expect(repository.deletedIds, isEmpty);

      await mouse.moveTo(tester.getCenter(clip));
      await _pumpHoverActions(tester);
      await tester.tap(more, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      await tester.tap(delete, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      await tester.tap(
        find.widgetWithText(Button, 'Delete'),
        kind: PointerDeviceKind.mouse,
      );
      await _pumpHoverActions(tester);
      expect(repository.deletedIds, ['actions']);
      expect(controller.window.items, isEmpty);
      expect(controller.selection.selectedId, isNull);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'keeps row actions hidden for touch input',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'touch',
              contentType: 'text',
              preview: 'Touch action clip',
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
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
      await tester.tap(find.text('Touch action clip'));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey<String>('history-row-pin-touch')),
        findsNothing,
      );
      expect(
        find.byKey(const ValueKey<String>('history-row-more-touch')),
        findsNothing,
      );
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets(
    'double-click, Enter and the copy shortcut copy a row with a toast',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'double',
              contentType: 'text',
              preview: 'Double-click clip',
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();
      final row = find.byKey(const ValueKey<String>('history-clip-double'));
      final point = tester.getTopLeft(row) + const Offset(40, 16);

      await tester.tapAt(point, kind: PointerDeviceKind.mouse);
      await tester.pump(const Duration(milliseconds: 50));
      expect(repository.copiedIds, isEmpty);
      await tester.tapAt(point, kind: PointerDeviceKind.mouse);
      await tester.pump();
      expect(controller.selection.selectedId, 'double');
      expect(repository.copiedIds, ['double']);
      expect(find.text('Copied'), findsOneWidget);

      final copyKey = defaultTargetPlatform == TargetPlatform.macOS
          ? LogicalKeyboardKey.metaLeft
          : LogicalKeyboardKey.controlLeft;
      await tester.sendKeyDownEvent(copyKey);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
      await tester.sendKeyUpEvent(copyKey);
      await tester.pump();
      expect(repository.copiedIds, ['double', 'double']);

      tester.widget<Button>(row).focusNode!.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      expect(repository.copiedIds, ['double', 'double', 'double']);
      expect(tester.takeException(), isNull);
      await tester.pump(const Duration(seconds: 6));
      await tester.pumpAndSettle();
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'context menu offers Copy first and Delete last',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'menu',
              contentType: 'text',
              preview: 'Context menu clip',
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();
      final row = find.byKey(const ValueKey<String>('history-clip-menu'));
      final copy = find.byKey(const ValueKey('history-menu-copy-menu'));
      final delete = find.byKey(const ValueKey('history-menu-delete-menu'));

      await tester.tap(
        row,
        buttons: kSecondaryButton,
        kind: PointerDeviceKind.mouse,
      );
      await _pumpHoverActions(tester);
      expect(copy, findsOneWidget);
      expect(delete, findsOneWidget);
      expect(tester.getCenter(copy).dy, lessThan(tester.getCenter(delete).dy));
      for (final item in [copy, delete]) {
        final icon = tester.getRect(
          find.descendant(of: item, matching: find.byType(Icon)),
        );
        final label = tester.getRect(
          find.descendant(of: item, matching: find.byType(Text)),
        );
        expect(icon.width, AppIconSize.sm);
        expect(label.left, greaterThanOrEqualTo(icon.right));
      }
      await tester.tap(copy, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      expect(repository.copiedIds, ['menu']);
      expect(find.text('Copied'), findsOneWidget);
      expect(controller.selection.selectedId, isNull);

      // macOS reserves Control-click for the menu instead of selection.
      if (defaultTargetPlatform == TargetPlatform.macOS) {
        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        await tester.tap(row, kind: PointerDeviceKind.mouse);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      } else {
        await tester.tap(
          row,
          buttons: kSecondaryButton,
          kind: PointerDeviceKind.mouse,
        );
      }
      await _pumpHoverActions(tester);
      expect(controller.bulk.isBulkSelecting, isFalse);
      await tester.tap(delete, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      expect(find.text('Delete this clip?'), findsOneWidget);
      expect(repository.deletedIds, isEmpty);
      await tester.tap(
        find.widgetWithText(Button, 'Delete'),
        kind: PointerDeviceKind.mouse,
      );
      await _pumpHoverActions(tester);
      expect(repository.deletedIds, ['menu']);
      expect(controller.window.items, isEmpty);
      await tester.pump(const Duration(seconds: 6));
      await _pumpHoverActions(tester);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  for (final scenario in [
    (platform: TargetPlatform.android, width: 480.0, pixelRatio: 1.0),
    (platform: TargetPlatform.windows, width: 900.0, pixelRatio: 2.0),
    (platform: TargetPlatform.macOS, width: 1400.0, pixelRatio: 2.0),
  ]) {
    for (final dimensions in [
      (1200, 200),
      (200, 1200),
      (1200, 1200),
      (60, 30),
    ]) {
      testWidgets(
        'fits ${dimensions.$1}x${dimensions.$2} list previews on ${scenario.platform.name}',
        (tester) async {
          await tester.binding.setSurfaceSize(Size(scenario.width, 900));
          addTearDown(() => tester.binding.setSurfaceSize(null));
          final repository = _ScreenRepository()
            ..fitImagePreviewBounds = true
            ..availableImagePreview = HistoryImagePreview(
              raster.encodePng(
                raster.Image(width: dimensions.$1, height: dimensions.$2),
              ),
              width: dimensions.$1,
              height: dimensions.$2,
            )
            ..page = HistoryClipPage(
              items: [
                HistoryClip(
                  id: 'proportional',
                  contentType: 'image/png',
                  preview: '[image]',
                  createdAt: DateTime.utc(2026),
                  pinned: false,
                  kind: HistoryClipKind.image,
                ),
              ],
            );
          final controller = HistoryController(repository);
          addTearDown(controller.dispose);
          await tester.pumpWidget(
            ShadcnApp(
              theme: AppTheme.light.copyWith(platform: () => scenario.platform),
              home: MediaQuery(
                data: MediaQueryData(devicePixelRatio: scenario.pixelRatio),
                child: Scaffold(child: HistoryScreen(controller: controller)),
              ),
            ),
          );
          await tester.pumpAndSettle();

          final image = find.byKey(
            const ValueKey<String>('history-card-image-proportional'),
          );
          final viewport = find
              .ancestor(of: image, matching: find.byType(LayoutBuilder))
              .first;
          final imageSize = tester.getSize(image);
          final availableWidth = tester.getSize(viewport).width;
          final imageContext = tester.element(image);
          final linePainter = TextPainter(
            text: TextSpan(
              text: 'Ag',
              style: DefaultTextStyle.of(imageContext).style,
            ),
            maxLines: 1,
            textDirection: Directionality.of(imageContext),
            textScaler: MediaQuery.textScalerOf(imageContext),
          )..layout();
          final maxHeight = linePainter.height * 8;

          expect(
            imageSize.width / imageSize.height,
            closeTo(dimensions.$1 / dimensions.$2, 0.05),
          );
          expect(imageSize.width, lessThanOrEqualTo(availableWidth));
          expect(imageSize.height, lessThanOrEqualTo(maxHeight));
          final expectedSize = applyBoxFit(
            BoxFit.scaleDown,
            Size(dimensions.$1.toDouble(), dimensions.$2.toDouble()),
            Size(availableWidth, maxHeight),
          ).destination;
          expect(imageSize.width, closeTo(expectedSize.width, 1));
          expect(imageSize.height, closeTo(expectedSize.height, 1));
          final expectedWidth = (availableWidth * scenario.pixelRatio)
              .ceil()
              .clamp(1, 2048);
          final expectedHeight = (maxHeight * scenario.pixelRatio).ceil().clamp(
            1,
            2048,
          );
          expect(repository.requestedImageEdges, [null]);
          expect(repository.requestedImageBounds.single?.width, expectedWidth);
          expect(
            repository.requestedImageBounds.single?.height,
            expectedHeight,
          );
          final provider = tester.widget<Image>(image).image as ResizeImage;
          expect(provider.width, lessThanOrEqualTo(expectedWidth));
          expect(provider.height, lessThanOrEqualTo(expectedHeight));
          expect(
            provider.width,
            lessThanOrEqualTo((imageSize.width * scenario.pixelRatio).ceil()),
          );
          expect(
            provider.height,
            lessThanOrEqualTo((imageSize.height * scenario.pixelRatio).ceil()),
          );
          expect(tester.takeException(), isNull);

          if (scenario.platform == TargetPlatform.macOS &&
              dimensions == (1200, 200)) {
            await tester.binding.setSurfaceSize(const Size(700, 900));
            await tester.pumpAndSettle();
            final resizedWidth = tester.getSize(viewport).width;
            final resizedImage = tester.getSize(image);
            expect(resizedImage.width, closeTo(resizedWidth, 0.01));
            expect(resizedImage.width / resizedImage.height, closeTo(6, 0.05));
            expect(repository.requestedImageBounds.length, 2);
            expect(
              repository.requestedImageBounds.last?.width,
              (resizedWidth * scenario.pixelRatio).ceil(),
            );
            expect(
              repository.requestedImageBounds.last?.height,
              expectedHeight,
            );
            expect(tester.takeException(), isNull);
          }
        },
      );
    }
  }

  testWidgets('shows the wide inspector only after a clip is selected', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'selectable',
            contentType: 'text/plain',
            preview: 'Selectable clip',
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    final list = find.byKey(const ValueKey<String>('history-clip-list'));
    expect(list, findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('history-detail-inspector')),
      findsNothing,
    );
    expect(find.text('Select a clip'), findsNothing);
    expect(tester.getSize(list).width, closeTo(1400 - AppSpacing.xxxl, 1));

    await tester.tap(find.text('Selectable clip'));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('history-detail-inspector')),
      findsOneWidget,
    );
    expect(tester.getSize(list).width, lessThan(1400 - AppSpacing.xxxl));
    expect(find.text('Observed app'), findsNothing);
    expect(
      find.byKey(const ValueKey<String>('history-detail-source-app')),
      findsNothing,
    );
    expect(repository.requestedSourceIconIds, isEmpty);

    final close = find.byKey(
      const ValueKey<String>('history-detail-inspector-close'),
    );
    expect(close, findsOneWidget);
    expect(find.bySemanticsLabel('Close clip details'), findsOneWidget);
    await tester.tap(close);
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('history-detail-inspector')),
      findsNothing,
    );
    expect(tester.getSize(list).width, closeTo(1400 - AppSpacing.xxxl, 1));
  });

  testWidgets('switches from the right inspector below 800 logical pixels', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(800, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'breakpoint',
            contentType: 'text/plain',
            preview: 'Breakpoint clip',
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Breakpoint clip'));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('history-detail-inspector')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('history-detail-drawer')),
      findsNothing,
    );

    await tester.binding.setSurfaceSize(const Size(799, 900));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Breakpoint clip'));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('history-detail-inspector')),
      findsNothing,
    );
    expect(
      find.byKey(const ValueKey<String>('history-detail-drawer')),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('resizes the wide inspector from its border without a handle', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'text',
            contentType: 'text/plain',
            preview: 'Resizable inspector clip',
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Resizable inspector clip'));
    await tester.pumpAndSettle();

    expect(find.text('Selected'), findsNothing);
    final selectedCardFinder = find.byKey(
      const ValueKey<String>('history-clip-text'),
    );
    final selectedCard = tester.widget<Button>(selectedCardFinder);
    final selectedCardContext = tester.element(selectedCardFinder);
    final inactiveDecoration = const ButtonStyle.ghost().decoration(
      selectedCardContext,
      const {},
    );
    final activeDecoration = selectedCard.style.decoration(
      selectedCardContext,
      const {},
    );
    expect(activeDecoration, isNot(inactiveDecoration));
    expect((inactiveDecoration as BoxDecoration).border, isNull);
    expect((activeDecoration as BoxDecoration).border, isNull);
    expect(find.byIcon(LucideIcons.gripVertical), findsNothing);
    final inspector = find.byKey(
      const ValueKey<String>('history-detail-inspector'),
    );
    final clipList = find.byKey(const ValueKey<String>('history-clip-list'));
    expect(inspector, findsOneWidget);
    expect(clipList, findsOneWidget);
    final initialRect = tester.getRect(inspector);
    final initialListRect = tester.getRect(clipList);
    expect(
      initialRect.left - initialListRect.right,
      closeTo(AppSpacing.lg, 0.01),
    );

    await tester.dragFrom(
      Offset(initialRect.left, initialRect.center.dy),
      const Offset(-80, 0),
    );
    await tester.pumpAndSettle();

    expect(tester.getSize(inspector).width, greaterThan(initialRect.width));
    expect(
      tester.getRect(inspector).left - tester.getRect(clipList).right,
      closeTo(AppSpacing.lg, 0.01),
    );
  });

  testWidgets(
    'keeps desktop metadata below actions and outside content scrolling',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'details',
              contentType: 'text/plain',
              preview: 'Desktop metadata clip',
              body: List<String>.filled(
                100,
                'Scrollable desktop detail content',
              ).join('\n'),
              createdAt: DateTime.utc(2026),
              pinned: false,
              sourceApp: 'Editor',
              sourceAppIconId: 'app:editor',
              file: const HistoryFileDetails(
                name: 'notes.txt',
                mimeType: 'text/plain',
                sizeBytes: 2048,
                fileCount: 2,
              ),
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('Desktop metadata clip'));
      await tester.pumpAndSettle();

      final scrollContent = find.byKey(
        const ValueKey<String>('history-detail-scroll-content'),
      );
      final actions = find.byKey(
        const ValueKey<String>('history-detail-actions'),
      );
      final metadata = find.byKey(
        const ValueKey<String>('history-detail-metadata'),
      );
      expect(scrollContent, findsOneWidget);
      expect(actions, findsOneWidget);
      expect(metadata, findsOneWidget);
      expect(find.byType(OutlineButton), findsNothing);
      expect(find.text('Copy plain text'), findsNothing);
      expect(find.bySemanticsLabel('Copy options'), findsOneWidget);
      expect(_detailAction('Pin'), findsOneWidget);
      expect(
        find.descendant(of: scrollContent, matching: actions),
        findsNothing,
      );
      expect(
        find.descendant(of: scrollContent, matching: metadata),
        findsNothing,
      );
      expect(
        tester.getBottomLeft(actions).dy,
        lessThan(tester.getTopLeft(metadata).dy),
      );
      final table = tester.widget<Table>(metadata);
      final tableContext = tester.element(metadata);
      final theme = Theme.of(tableContext);
      expect(table.theme, isNull);
      for (final row in table.rows!) {
        final border = row
            .buildDefaultTheme(tableContext)
            .border!
            .resolve(const <WidgetState>{})!;
        expect(border.top.style, BorderStyle.none);
        expect(border.left.style, BorderStyle.none);
        expect(border.right.style, BorderStyle.none);
        expect(border.bottom.color, theme.colorScheme.border);
        expect(border.bottom.width, 1);
      }
      for (final label in [
        'Captured',
        'Observed app',
        'Clip type',
        'File',
        'Type',
        'Size',
        'Files',
      ]) {
        expect(
          find.descendant(of: metadata, matching: find.text(label)),
          findsOneWidget,
        );
      }
      final sourceApp = find.byKey(
        const ValueKey<String>('history-detail-source-app'),
      );
      expect(sourceApp, findsOneWidget);
      expect(tester.widget<SourceAppLabel>(sourceApp).name, 'Editor');
      expect(
        find.descendant(of: sourceApp, matching: find.byType(Avatar)),
        findsOneWidget,
      );
      expect(
        find.descendant(of: metadata, matching: find.text('notes.txt')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: metadata, matching: find.text('2.0 KiB')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: metadata, matching: find.text('Sync')),
        findsNothing,
      );
      expect(
        tester
            .widget<Text>(
              find.descendant(of: metadata, matching: find.text('Captured')),
            )
            .style
            ?.fontSize,
        AppTypographySize.inspectorMetadata,
      );
      expect(
        tester.widget<SourceAppLabel>(sourceApp).style?.fontSize,
        AppTypographySize.inspectorMetadata,
      );

      final actionsTop = tester.getTopLeft(actions).dy;
      final metadataTop = tester.getTopLeft(metadata).dy;
      final scrollable = find.descendant(
        of: scrollContent,
        matching: find.byType(Scrollable),
      );
      final scrollState = tester.state<ScrollableState>(scrollable.first);
      expect(scrollState.position.maxScrollExtent, greaterThan(0));
      await tester.drag(scrollContent, const Offset(0, -300));
      await tester.pump();

      expect(scrollState.position.pixels, greaterThan(0));
      expect(tester.getTopLeft(actions).dy, actionsTop);
      expect(tester.getTopLeft(metadata).dy, metadataTop);
    },
  );

  testWidgets('places the selected clip kind in the shared control container', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'image-kind',
            contentType: 'image/png',
            preview: '[image]',
            createdAt: DateTime.utc(2026),
            pinned: false,
            kind: HistoryClipKind.image,
            origin: 'Work Mac',
            originDeviceClass: DeviceClass.laptop,
            image: const HistoryImageDetails(
              width: 1920,
              height: 1080,
              sizeBytes: 4 * 1024 * 1024,
            ),
            tooLargeToSync: true,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(
      find.byKey(const ValueKey<String>('history-clip-image-kind')),
    );
    await tester.pumpAndSettle();

    final container = find.byKey(
      const ValueKey<String>('history-detail-kind-icon'),
    );
    final card = tester.widget<Card>(
      find.descendant(of: container, matching: find.byType(Card)),
    );
    final theme = Theme.of(tester.element(container));
    expect(tester.getSize(container), const Size.square(AppControlSize.large));
    expect(card.theme?.filled, isTrue);
    expect(card.theme?.fillColor, theme.colorScheme.secondary);
    expect(card.theme?.borderRadius, theme.borderRadiusMd);
    expect(card.theme?.borderWidth, AppSpacing.zero);
    expect(
      tester
          .widget<Icon>(
            find.descendant(
              of: container,
              matching: find.byIcon(LucideIcons.image),
            ),
          )
          .size,
      AppIconSize.sm,
    );
    final metadata = find.byKey(
      const ValueKey<String>('history-detail-metadata'),
    );
    for (final entry in <String, String>{
      'Clip type': 'Image',
      'Type': 'image/png',
      'Resolution': '1920 × 1080',
      'Size': '4.0 MiB',
      'Sync': 'Too large to sync',
    }.entries) {
      expect(
        find.descendant(of: metadata, matching: find.text(entry.key)),
        findsOneWidget,
      );
      expect(
        find.descendant(of: metadata, matching: find.text(entry.value)),
        findsOneWidget,
      );
    }
    final device = find.byKey(const ValueKey<String>('history-detail-device'));
    expect(
      find.descendant(of: metadata, matching: find.text('Device')),
      findsOneWidget,
    );
    expect(device, findsOneWidget);
    expect(tester.widget<DeviceLabel>(device).name, 'Work Mac');
    expect(
      find.descendant(of: device, matching: find.byIcon(LucideIcons.laptop)),
      findsOneWidget,
    );
    expect(
      find.descendant(of: device, matching: find.byType(Avatar)),
      findsNothing,
    );
    final syncWarning = tester.widget<SelectableText>(
      find.descendant(
        of: metadata,
        matching: find.byWidgetPredicate(
          (widget) =>
              widget is SelectableText && widget.data == 'Too large to sync',
        ),
      ),
    );
    expect(syncWarning.style?.color, theme.colorScheme.destructive);
  });

  testWidgets('keeps fitting desktop text at its intrinsic height', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1100));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'short-text',
            contentType: 'text/plain',
            preview: 'shadcn',
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('shadcn'));
    await tester.pumpAndSettle();

    final scrollContent = find.byKey(
      const ValueKey<String>('history-detail-scroll-content'),
    );
    final detailText = find.descendant(
      of: scrollContent,
      matching: find.text('shadcn'),
    );
    final actions = find.byKey(
      const ValueKey<String>('history-detail-actions'),
    );
    expect(detailText, findsOneWidget);
    expect(
      tester.getTopLeft(actions).dy - tester.getBottomLeft(detailText).dy,
      closeTo(AppSpacing.lg, 0.01),
    );
    final scrollable = find.descendant(
      of: scrollContent,
      matching: find.byType(Scrollable),
    );
    expect(
      tester.state<ScrollableState>(scrollable.first).position.maxScrollExtent,
      0,
    );
  });

  testWidgets('keeps a fitting desktop image at its natural preview size', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1100));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..availableImagePreview = HistoryImagePreview(
        Uint8List.fromList([0, 1, 2, 3]),
        width: 240,
        height: 90,
      )
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'image',
            contentType: 'image/png',
            preview: '[image]',
            createdAt: DateTime.utc(2026),
            pinned: false,
            kind: HistoryClipKind.image,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey<String>('history-clip-image')));
    await tester.pumpAndSettle();

    final inspector = find.byKey(
      const ValueKey<String>('history-detail-inspector'),
    );
    final imageViewport = find.byKey(
      const ValueKey<String>('history-detail-image-viewport'),
    );
    final actions = find.byKey(
      const ValueKey<String>('history-detail-actions'),
    );
    expect(imageViewport, findsOneWidget);
    final initialInspectorRect = tester.getRect(inspector);
    final initialViewportSize = tester.getSize(imageViewport);
    expect(initialViewportSize, const Size(240, 90));
    expect(
      tester.getTopLeft(actions).dy - tester.getBottomLeft(imageViewport).dy,
      closeTo(AppSpacing.lg, 0.01),
    );

    await tester.dragFrom(
      Offset(initialInspectorRect.left, initialInspectorRect.center.dy),
      const Offset(-80, 0),
    );
    await tester.pumpAndSettle();

    expect(tester.getSize(imageViewport), initialViewportSize);
  });

  testWidgets('opens a selected clip in a full-width bottom drawer', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(480, 800);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'text',
            contentType: 'text/plain',
            preview: 'A selected text clip',
            body: List<String>.filled(
              80,
              'Scrollable detail content',
            ).join('\n'),
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    final drawerVisibility = <bool>[];
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(
          child: SizedBox(
            width: 480,
            height: 800,
            child: HistoryScreen(
              controller: controller,
              onDrawerVisibilityChanged: drawerVisibility.add,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('A selected text clip'), findsOneWidget);
    await tester.tap(find.text('A selected text clip'));
    await tester.pumpAndSettle();

    final drawer = find.byKey(const ValueKey<String>('history-detail-drawer'));
    expect(drawer, findsOneWidget);
    expect(find.byType(DrawerWrapper), findsOneWidget);
    expect(find.byType(AlertDialog), findsNothing);
    expect(drawerVisibility, [isTrue]);
    final drawerSize = tester.getSize(drawer);
    final mediaSize = MediaQuery.sizeOf(tester.element(drawer));
    expect(drawerSize.width, closeTo(480, 2));
    expect(
      drawerSize.height,
      mediaSize.height * AppOverlaySize.drawerHeightFactor,
    );
    expect(tester.getBottomRight(find.byType(DrawerWrapper)).dy, 800);
    expect(find.text('Copy plain text'), findsNothing);
    expect(find.bySemanticsLabel('Copy options'), findsOneWidget);
    final detailCard = find.byKey(
      const ValueKey<String>('history-detail-drawer-card'),
    );
    final scrollContent = find.byKey(
      const ValueKey<String>('history-detail-scroll-content'),
    );
    final actions = find.byKey(
      const ValueKey<String>('history-detail-actions'),
    );
    final close = find.byKey(
      const ValueKey<String>('history-detail-drawer-close'),
    );
    expect(find.descendant(of: detailCard, matching: close), findsNothing);
    expect(find.descendant(of: scrollContent, matching: actions), findsNothing);
    final metadata = find.byKey(
      const ValueKey<String>('history-detail-metadata'),
    );
    expect(
      find.descendant(of: scrollContent, matching: metadata),
      findsOneWidget,
    );
    final actionsTop = tester.getTopLeft(actions).dy;
    await tester.drag(scrollContent, const Offset(0, -300));
    await tester.pump();
    expect(tester.getTopLeft(actions).dy, actionsTop);

    await tester.tap(close);
    await tester.pumpAndSettle();
    expect(drawer, findsNothing);
    expect(drawerVisibility, [isTrue, isFalse]);
  });

  testWidgets('keeps the mobile image fixed while its metadata table scrolls', (
    tester,
  ) async {
    // Exercise overflow on a compact viewport instead of relying on a larger
    // metadata font to make short values scroll.
    await tester.binding.setSurfaceSize(const Size(480, 640));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..availableImagePreview = HistoryImagePreview(
        Uint8List.fromList([0, 1, 2, 3]),
        width: 240,
        height: 1200,
      )
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'tall-mobile-image',
            contentType: 'image/png',
            preview: '[image]',
            createdAt: DateTime.utc(2026),
            pinned: false,
            kind: HistoryClipKind.image,
            origin: 'Work Mac',
            originDeviceClass: DeviceClass.laptop,
            sourceApp: 'Editor',
            sourceAppIconId: 'app:editor',
            image: const HistoryImageDetails(
              width: 240,
              height: 1200,
              sizeBytes: 4096,
            ),
            tooLargeToSync: true,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(
      find.byKey(const ValueKey<String>('history-clip-tall-mobile-image')),
    );
    await tester.pumpAndSettle();

    final drawer = find.byKey(const ValueKey<String>('history-detail-drawer'));
    final fittedContent = find.byKey(
      const ValueKey<String>('history-detail-image-fit-content'),
    );
    final scrollContent = find.byKey(
      const ValueKey<String>('history-detail-scroll-metadata'),
    );
    final imageViewport = find.byKey(
      const ValueKey<String>('history-detail-image-viewport'),
    );
    final actions = find.byKey(
      const ValueKey<String>('history-detail-actions'),
    );
    expect(drawer, findsOneWidget);
    expect(scrollContent, findsOneWidget);
    expect(imageViewport, findsOneWidget);
    expect(
      find.ancestor(of: imageViewport, matching: find.byType(Scrollable)),
      findsNothing,
    );
    final metadata = find.byKey(
      const ValueKey<String>('history-detail-metadata'),
    );
    expect(
      find.descendant(of: scrollContent, matching: metadata),
      findsOneWidget,
    );
    final table = tester.widget<Table>(metadata);
    final tableContext = tester.element(metadata);
    for (final row in table.rows!) {
      final border = row
          .buildDefaultTheme(tableContext)
          .border!
          .resolve(const <WidgetState>{})!;
      expect(border.bottom.color, Theme.of(tableContext).colorScheme.border);
      expect(border.bottom.width, 1);
    }
    final device = find.descendant(
      of: scrollContent,
      matching: find.byKey(const ValueKey<String>('history-detail-device')),
    );
    expect(device, findsOneWidget);
    expect(tester.widget<DeviceLabel>(device).name, 'Work Mac');
    expect(
      find.descendant(of: device, matching: find.byIcon(LucideIcons.laptop)),
      findsOneWidget,
    );
    final scrollable = find.descendant(
      of: scrollContent,
      matching: find.byType(Scrollable),
    );
    final scrollState = tester.state<ScrollableState>(scrollable.first);
    expect(scrollState.position.maxScrollExtent, greaterThan(0));
    expect(find.descendant(of: scrollContent, matching: actions), findsNothing);
    expect(
      tester.getTopLeft(imageViewport).dy,
      greaterThanOrEqualTo(tester.getTopLeft(fittedContent).dy),
    );
    expect(
      tester.getBottomLeft(imageViewport).dy,
      lessThanOrEqualTo(tester.getTopLeft(scrollContent).dy),
    );
    expect(
      tester.getBottomLeft(imageViewport).dy,
      lessThan(tester.getTopLeft(actions).dy),
    );
    expect(tester.getSize(imageViewport).height, lessThan(1200));
    expect(
      tester.getBottomLeft(actions).dy,
      lessThanOrEqualTo(tester.getBottomLeft(drawer).dy),
    );
    final actionsRect = tester.getRect(actions);
    final imageRect = tester.getRect(imageViewport);
    final metadataTop = tester.getTopLeft(metadata).dy;
    await tester.drag(scrollContent, const Offset(0, -300));
    await tester.pumpAndSettle();
    expect(scrollState.position.pixels, greaterThan(0));
    expect(tester.getRect(imageViewport), imageRect);
    expect(tester.getTopLeft(metadata).dy, lessThan(metadataTop));
    expect(tester.getRect(actions), actionsRect);
    expect(_detailAction('Copy').hitTestable(), findsOneWidget);
    expect(_detailAction('Pin').hitTestable(), findsOneWidget);
    expect(_detailAction('Delete').hitTestable(), findsOneWidget);

    await tester.binding.setSurfaceSize(const Size(700, 480));
    await tester.pumpAndSettle();

    expect(tester.getTopLeft(drawer).dy, greaterThanOrEqualTo(0));
    expect(tester.getBottomLeft(drawer).dy, lessThanOrEqualTo(480));
    expect(
      tester.getBottomLeft(imageViewport).dy,
      lessThan(tester.getTopLeft(actions).dy),
    );
    expect(
      tester.getBottomLeft(actions).dy,
      lessThanOrEqualTo(tester.getBottomLeft(drawer).dy),
    );
    expect(tester.takeException(), isNull);
  });

  for (final size in [const Size(320, 568), const Size(700, 360)]) {
    testWidgets('anchors short mobile clip actions at the bottom at $size', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(size);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'short-mobile',
              contentType: 'text/plain',
              preview: 'Short mobile clip',
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Short mobile clip'));
      await tester.pumpAndSettle();

      final card = find.byKey(
        const ValueKey<String>('history-detail-drawer-card'),
      );
      final actions = find.byKey(
        const ValueKey<String>('history-detail-actions'),
      );
      final kindIcon = find.byKey(
        const ValueKey<String>('history-detail-kind-icon'),
      );
      expect(
        tester.getSize(kindIcon),
        const Size.square(AppControlSize.compact),
      );
      final heading = find.descendant(
        of: find.byKey(const ValueKey<String>('history-detail-heading')),
        matching: find.text('Text'),
      );
      expect(
        tester.widget<Text>(heading).style?.fontSize,
        Theme.of(tester.element(heading)).typography.small.fontSize,
      );
      final cardBody = find.descendant(of: card, matching: find.byType(Column));
      expect(
        tester.getBottomLeft(cardBody.first).dy -
            tester.getBottomLeft(actions).dy,
        AppSpacing.xs,
      );
      for (final label in ['Copy', 'Pin', 'Delete']) {
        expect(_detailAction(label).hitTestable(), findsOneWidget);
      }
      expect(find.text('Copy plain text'), findsNothing);
      expect(
        find
            .byKey(const ValueKey<String>('history-copy-options'))
            .hitTestable(),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('renders backend facet labels without exposing their IDs', (
    tester,
  ) async {
    final repository = _ScreenRepository()
      ..availableFacets = const HistoryFacets(
        originDevices: [
          HistoryDeviceFacet(
            id: 'device-1',
            label: 'Work Mac',
            deviceClass: DeviceClass.laptop,
          ),
        ],
        sourceApps: [
          HistorySourceAppFacet(
            id: 'com.example.editor',
            label: 'Editor',
            iconId: 'app:com.example.editor',
          ),
        ],
      )
      ..sourceIcons['app:com.example.editor'] = HistorySourceAppIcon(
        base64Decode(
          'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=',
        ),
      );
    final controller = HistoryController(repository);
    await tester.pumpWidget(
      ShadcnApp(
        home: SizedBox(
          width: 480,
          height: 800,
          child: HistoryScreen(controller: controller),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.bySemanticsLabel('All devices'));
    await tester.pump(const Duration(milliseconds: 200));
    final workMac = find.byWidgetPredicate(
      (widget) => widget is DeviceLabel && widget.name == 'Work Mac',
    );
    expect(workMac, findsOneWidget);
    expect(find.byIcon(LucideIcons.laptop), findsOneWidget);
    expect(find.text('device-1'), findsNothing);
    expect(find.text('com.example.editor'), findsNothing);
    await tester.tap(workMac);
    await tester.pumpAndSettle();

    await tester.tap(find.bySemanticsLabel('All apps'));
    await tester.pump(const Duration(milliseconds: 200));
    await tester.pump(const Duration(milliseconds: 1));
    final editorLabel = find.byWidgetPredicate(
      (widget) => widget is SourceAppLabel && widget.name == 'Editor',
    );
    expect(editorLabel, findsOneWidget);
    final editorAvatar = tester.widget<Avatar>(
      find.descendant(of: editorLabel, matching: find.byType(Avatar)),
    );
    expect(
      editorAvatar.provider,
      isA<ResizeImage>().having(
        (image) => image.imageProvider,
        'source',
        isA<MemoryImage>(),
      ),
    );
    expect(repository.requestedSourceIconIds, ['app:com.example.editor']);
    expect(find.text('com.example.editor'), findsNothing);
    controller.dispose();
  });

  testWidgets('shows skipped rows when an empty history page is unreadable', (
    tester,
  ) async {
    final repository = _ScreenRepository()
      ..page = const HistoryClipPage(items: [], skippedUndecryptable: 2);
    final controller = HistoryController(repository);
    await tester.pumpWidget(
      ShadcnApp(
        home: SizedBox(
          width: 480,
          height: 800,
          child: HistoryScreen(controller: controller),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('2 clipboard rows were skipped'), findsOneWidget);
    expect(find.text('No clips found'), findsOneWidget);
    controller.dispose();
  });

  testWidgets(
    'Mod+N on desktop and + on Android open the new clip editor',
    (tester) async {
      final repository = _EditScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'text',
              contentType: 'text/plain',
              preview: 'Existing clip',
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.binding.setSurfaceSize(const Size(1200, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await _pumpHoverActions(tester);

      final newClip = find.byKey(const ValueKey('history-new-clip'));
      if (defaultTargetPlatform == TargetPlatform.android) {
        await tester.tap(newClip);
      } else {
        expect(newClip, findsNothing);
        await tester.tap(find.byType(TextField).first);
        await tester.pump();
        final modifier = defaultTargetPlatform == TargetPlatform.macOS
            ? LogicalKeyboardKey.metaLeft
            : LogicalKeyboardKey.controlLeft;
        await tester.sendKeyDownEvent(modifier);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
        await tester.sendKeyUpEvent(modifier);
      }
      await _pumpHoverActions(tester);
      expect(find.byKey(const ValueKey('clip-edit-drawer')), findsOneWidget);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets('keeps every compact icon-only toolbar control square', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = HistoryController(_ScreenRepository());
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
        builder: AppTheme.builder,
        home: HistoryScreen(controller: controller),
      ),
    );
    await tester.pumpAndSettle();

    final searchButton = find.byKey(
      const ValueKey<String>('history-search-toggle'),
    );
    final controls = <Element>[
      ...searchButton.evaluate(),
      ...find.byKey(const ValueKey<String>('history-import-files')).evaluate(),
      ...find.byWidgetPredicate((widget) => widget is Select).evaluate(),
    ];
    expect(searchButton, findsOneWidget);
    expect(
      find.byWidgetPredicate((widget) => widget is Select),
      findsNWidgets(5),
    );
    expect(find.bySemanticsLabel('All pins'), findsNothing);
    expect(find.bySemanticsLabel('Pinned only'), findsNothing);
    final controlHeight = tester.getSize(searchButton).height;
    expect(controlHeight, greaterThanOrEqualTo(AppControlSize.large));
    for (final control in controls) {
      final size = tester.getSize(find.byWidget(control.widget));
      expect(size, Size.square(controlHeight));
    }

    final search = tester.widget<Button>(searchButton);
    final searchContext = tester.element(searchButton);
    final searchDecoration =
        search.style.decoration(searchContext, const {}) as BoxDecoration;
    final firstSelectFinder = find
        .byWidgetPredicate((widget) => widget is Select)
        .first;
    final selectContext = tester.element(firstSelectFinder);
    final selectTheme = ComponentTheme.maybeOf<SelectTheme>(selectContext)!;
    final selectDecoration =
        selectTheme.decoration!(
              tester.element(firstSelectFinder),
              const {},
              const BoxDecoration(),
            )
            as BoxDecoration;
    expect(searchDecoration.color, selectDecoration.color);
    expect(searchDecoration.borderRadius, selectDecoration.borderRadius);
  });

  testWidgets(
    'compact search replaces all actions and keeps their height',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      for (final width in [320.0, 390.0]) {
        tester.view.physicalSize = Size(width, 800);
        for (final scale in [1.0, 1.5, 2.0]) {
          final controller = HistoryController(_ScreenRepository());
          addTearDown(controller.dispose);
          await tester.pumpWidget(
            ShadcnApp(
              theme: AppTheme.light,
              builder: (context, child) => MediaQuery(
                data: MediaQuery.of(
                  context,
                ).copyWith(textScaler: TextScaler.linear(scale)),
                child: Builder(
                  builder: (context) => AppTheme.builder(context, child),
                ),
              ),
              home: HistoryScreen(key: UniqueKey(), controller: controller),
            ),
          );
          await tester.pumpAndSettle();

          final searchToggle = find.byKey(
            const ValueKey<String>('history-search-toggle'),
          );
          final actionHeight = tester.getRect(searchToggle).height;
          final selection = find.byKey(const ValueKey('history-select-clips'));
          expect(
            tester.getRect(selection).right,
            lessThan(tester.getRect(searchToggle).left),
          );
          final toolbarWidth = width - AppSpacing.lg * 2;
          await tester.tap(searchToggle);
          await tester.pumpAndSettle();

          final searchField = find.byType(TextField);
          expect(
            tester.getRect(searchField).height,
            closeTo(actionHeight, 0.01),
            reason: 'Width $width, text scale $scale',
          );
          expect(
            tester.getRect(searchField).width,
            closeTo(toolbarWidth, 0.01),
          );
          expect(selection, findsNothing);
          expect(
            find.byKey(const ValueKey<String>('history-import-files')),
            findsNothing,
          );
          expect(
            find.byWidgetPredicate((widget) => widget is Select),
            findsNothing,
          );
          await tester.enterText(searchField, 'Example');
          await tester.pump(const Duration(milliseconds: 300));
          expect(controller.window.query.search, 'Example');
          await tester.tap(find.bySemanticsLabel('Close search'));
          await tester.pump(const Duration(milliseconds: 300));
          await tester.pumpAndSettle();

          expect(controller.window.query.search, isEmpty);
          expect(searchToggle, findsOneWidget);
          expect(selection, findsOneWidget);
          expect(
            find.byWidgetPredicate((widget) => widget is Select),
            findsNWidgets(5),
          );
          expect(tester.takeException(), isNull);
        }
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets(
    'collapses search before toolbar controls wrap onto a second row',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(480, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = HistoryController(_ScreenRepository());
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          home: SizedBox(
            width: 480,
            height: 800,
            child: HistoryScreen(controller: controller),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(TextField), findsNothing);
      final search = find.byKey(const ValueKey('history-search-toggle'));
      expect(search, findsOneWidget);
      final baseline = tester.getRect(search);
      final controls = [
        find.byKey(const ValueKey('history-select-clips')),
        find.byKey(const ValueKey('history-date-filter')),
        find.byKey(const ValueKey('history-import-files')),
        ...find
            .byWidgetPredicate((widget) => widget is Select)
            .evaluate()
            .map((element) => find.byWidget(element.widget)),
      ];
      for (final control in controls) {
        final rect = tester.getRect(control);
        expect(rect.top, closeTo(baseline.top, 0.01));
        expect(rect.right, lessThanOrEqualTo(480 - AppSpacing.lg));
      }
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets(
    'preserves the search minimum at the labeled filter breakpoint',
    (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = HistoryController(_ScreenRepository());
      addTearDown(controller.dispose);
      for (final textScale in [1.0, 1.5, 2.0]) {
        await tester.binding.setSurfaceSize(const Size(3200, 800));
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            darkTheme: AppTheme.dark,
            themeMode: ThemeMode.light,
            builder: (context, child) => AppTheme.builder(
              context,
              MediaQuery(
                data: MediaQuery.of(
                  context,
                ).copyWith(textScaler: TextScaler.linear(textScale)),
                child: child!,
              ),
            ),
            home: HistoryScreen(controller: controller),
          ),
        );
        await tester.pumpAndSettle();

        final selects = find.byWidgetPredicate((widget) => widget is Select);
        expect(
          selects.evaluate().every(
            (element) => (element.widget as Select).expandIcon != null,
          ),
          isTrue,
        );
        final filterWidth = selects.evaluate().fold<double>(
          0,
          (sum, element) =>
              sum + tester.getSize(find.byWidget(element.widget)).width,
        );
        final breakpoint =
            160 +
            filterWidth +
            tester
                .getSize(
                  find.byKey(const ValueKey<String>('history-import-files')),
                )
                .width +
            tester
                .getSize(find.byKey(const ValueKey('history-select-clips')))
                .width +
            (AppSpacing.sm * 6) +
            (AppSpacing.lg * 2);
        for (final width in [breakpoint + 1, breakpoint, breakpoint - 1]) {
          await tester.binding.setSurfaceSize(Size(width, 800));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          expect(find.byType(TextField), findsOneWidget);
          expect(
            tester.getSize(find.byType(TextField)).width,
            greaterThanOrEqualTo(160),
            reason: 'Window width $width, text scale $textScale',
          );
          final height = tester
              .getSize(
                find.byKey(const ValueKey<String>('history-import-files')),
              )
              .height;
          expect(
            tester.getSize(find.byType(TextField)).height,
            closeTo(height, 0.01),
          );
          for (final select in selects.evaluate()) {
            expect(
              tester.getSize(find.byWidget(select.widget)).height,
              closeTo(height, 0.01),
            );
          }
        }
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets('shows only the standardized kind icon below the breakpoint', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(799, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..availableFacets = const HistoryFacets(
        tags: [HistoryTag(id: 'tag-work', name: 'Work')],
      )
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'meta',
            contentType: 'text',
            preview: 'Content stays at its normal size',
            createdAt: DateTime.utc(2026),
            pinned: true,
            tagIds: const ['tag-work', 'tag-removed'],
            kind: HistoryClipKind.text,
            sourceApp: 'Editor',
            sourceAppIconId: 'app:editor',
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
        home: SizedBox(
          width: 799,
          height: 800,
          child: HistoryScreen(controller: controller),
        ),
      ),
    );
    await tester.pumpAndSettle();

    final metaFinder = find.byKey(
      const ValueKey<String>('history-clip-meta-meta'),
    );
    final meta = tester.widget<Text>(metaFinder);
    expect(meta.textSpan?.toPlainText(), isNot(contains('Text')));
    expect(meta.textSpan?.toPlainText(), endsWith(' Work'));
    expect(
      find.descendant(of: metaFinder, matching: find.byIcon(LucideIcons.tag)),
      findsOneWidget,
    );
    final kindIcon = find.descendant(
      of: metaFinder,
      matching: find.byIcon(LucideIcons.type),
    );
    expect(kindIcon, findsOneWidget);
    expect(tester.widget<Icon>(kindIcon).size, AppIconSize.xs);
    expect(meta.style?.fontSize, AppTypographySize.historyMetadata);
    expect(meta.style?.height, 4 / 3);
    expect(tester.getSize(metaFinder).height, 16);
    expect(
      tester.getSize(find.byType(Avatar)),
      const Size.square(AppIconSize.xs),
    );
    expect(
      find.descendant(
        of: find.byKey(const ValueKey<String>('history-clip-meta')),
        matching: find.byIcon(LucideIcons.pin),
      ),
      findsNothing,
    );
  });

  testWidgets('notes the line count only on rows that certainly hide lines', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(799, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final semantics = tester.ensureSemantics();
    HistoryClip clip(
      String id,
      String preview, {
      bool truncated = false,
      int? lineCount,
    }) => HistoryClip(
      id: id,
      contentType: 'text',
      preview: preview,
      createdAt: DateTime.utc(2026),
      pinned: false,
      kind: id == 'code' ? HistoryClipKind.code : HistoryClipKind.text,
      truncated: truncated,
      lineCount: lineCount,
    );
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          clip('cut', 'one\ntwo\nthree\nfour\nfive'),
          clip('code', 'a();\r\nb();\r\nc();\r\nd();\r\ne();\r\nf();'),
          clip('fits', 'one\ntwo\nthree\nfour'),
          // One logical line wraps past the row limit: there is no line count
          // to report.
          clip('wrapped', 'word ' * 150),
          // A cut preview is never counted; only the runtime's count is used.
          clip('long', 'one\ntwo\nthree\nfour\nfive\nsix', truncated: true),
          clip(
            'counted',
            'one\ntwo\nthree\nfour\nfive',
            truncated: true,
            lineCount: 120,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    String meta(String id) => tester
        .widget<Text>(find.byKey(ValueKey<String>('history-clip-meta-$id')))
        .textSpan!
        .toPlainText(includeSemanticsLabels: false);
    expect(meta('cut'), endsWith(' • 5 lines'));
    expect(meta('code'), endsWith(' • 6 lines'));
    expect(meta('counted'), endsWith(' • 120 lines'));
    for (final id in ['fits', 'wrapped', 'long']) {
      expect(meta(id), isNot(contains('lines')));
    }
    // The note is visual only: the preview is read to assistive technology
    // in full, so the count is not announced.
    expect(find.bySemanticsLabel(RegExp('5 lines')), findsNothing);

    // The note yields inside the single metadata line on a phone at 2x text.
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    await tester.binding.setSurfaceSize(const Size(360, 900));
    await tester.pumpAndSettle();
    expect(meta('cut'), endsWith(' • 5 lines'));
    expect(
      tester
          .getSize(find.byKey(const ValueKey<String>('history-clip-meta-cut')))
          .height,
      32,
    );
    expect(tester.takeException(), isNull);
    semantics.dispose();
  });

  test('counts clip lines only where the count is complete and public', () {
    HistoryClip clip(
      String preview, {
      HistoryClipKind kind = HistoryClipKind.text,
      bool secret = false,
      bool truncated = false,
      int? lineCount,
    }) => HistoryClip(
      id: 'lines',
      contentType: 'text',
      preview: preview,
      createdAt: DateTime.utc(2026),
      pinned: false,
      kind: kind,
      secret: secret,
      truncated: truncated,
      lineCount: lineCount,
    );
    const five = 'a\nb\n\nd\ne';
    expect(HistoryClipPresentation.cutLineCount(clip(five)), 5);
    expect(
      HistoryClipPresentation.cutLineCount(clip(five, secret: true)),
      null,
    );
    expect(
      HistoryClipPresentation.cutLineCount(
        clip(five, kind: HistoryClipKind.file),
      ),
      null,
    );
    // A lone carriage return is not a guaranteed displayed line break.
    expect(HistoryClipPresentation.cutLineCount(clip('a\rb\rc\rd\re')), null);
    // The runtime counts the whole text, so a cut preview still has a count.
    expect(
      HistoryClipPresentation.cutLineCount(
        clip('a', truncated: true, lineCount: 9),
      ),
      9,
    );
    expect(
      HistoryClipPresentation.cutLineCount(clip(five, truncated: true)),
      null,
    );
    expect(
      HistoryClipPresentation.cutLineCount(clip(five, lineCount: 4)),
      null,
    );
    expect(
      HistoryClipPresentation.cutLineCount(
        clip(five, secret: true, lineCount: 9),
      ),
      null,
    );

    final uk = lookupAppLocalizations(const Locale('uk'));
    expect(uk.historyClipLineCount(5), '5 рядків');
    expect(uk.historyClipLineCount(21), '21 рядок');
    expect(uk.historyClipLineCount(22), '22 рядки');
    final en = lookupAppLocalizations(const Locale('en'));
    expect(en.historyClipLineCount(5), '5 lines');
  });

  testWidgets('shows the standardized kind icon and label at the wide size', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'wide-meta',
            contentType: 'text',
            preview: 'Desktop clip content',
            createdAt: DateTime.utc(2026),
            pinned: false,
            kind: HistoryClipKind.text,
            sourceApp: 'Editor',
            sourceAppIconId: 'app:editor',
            origin: 'Work Mac',
            originDeviceClass: DeviceClass.laptop,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    final metaFinder = find.byKey(
      const ValueKey<String>('history-clip-meta-wide-meta'),
    );
    final meta = tester.widget<Text>(metaFinder);
    expect(meta.style?.fontSize, 12);
    expect(meta.style?.height, 4 / 3);
    expect(tester.getSize(metaFinder).height, 16);
    expect(
      tester.widget<SourceAppLabel>(find.byType(SourceAppLabel)).iconSize,
      AppIconSize.xs,
    );
    expect(
      tester.widget<DeviceLabel>(find.byType(DeviceLabel)).iconSize,
      AppIconSize.xs,
    );
    expect(meta.textSpan?.toPlainText(), contains('Text'));
    expect(
      find.descendant(of: metaFinder, matching: find.byIcon(LucideIcons.type)),
      findsOneWidget,
    );
  });

  testWidgets('offers every semantic kind and sends the selected filter', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(2400, 1000));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository();
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('All clips'));
    await tester.pump(const Duration(milliseconds: 200));

    for (final label in ['Text', 'Link', 'Email', 'Color', 'Phone', 'Code']) {
      expect(find.text(label), findsOneWidget, reason: label);
    }

    await tester.scrollUntilVisible(
      find.text('Credentials'),
      100,
      scrollable: find
          .descendant(
            of: find.byWidgetPredicate((widget) => widget is SelectPopup),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await tester.tap(find.text('Credentials'));
    await tester.pump(const Duration(milliseconds: 200));
    expect(repository.queries.last.kind, HistoryClipKind.credentials);
  });

  testWidgets('facet dropdowns use no Radix overflow icons', (tester) async {
    final originDevices = List<HistoryDeviceFacet>.generate(
      40,
      (index) => HistoryDeviceFacet(
        id: 'device-$index',
        label: 'Device $index',
        deviceClass: DeviceClass.unknown,
      ),
    );
    final sourceApps = List<HistorySourceAppFacet>.generate(
      40,
      (index) => HistorySourceAppFacet(id: 'app-$index', label: 'App $index'),
    );
    final repository = _ScreenRepository()
      ..availableFacets = HistoryFacets(
        originDevices: originDevices,
        sourceApps: sourceApps,
      );
    final controller = HistoryController(repository);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(
          child: SizedBox(
            width: 480,
            height: 800,
            child: HistoryScreen(controller: controller),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.bySemanticsLabel('All devices'));
    await tester.pump(const Duration(milliseconds: 200));
    _expectNoRadixIcons(tester);
    await tester.tap(
      find.byWidgetPredicate(
        (widget) => widget is DeviceLabel && widget.name == 'Device 0',
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.bySemanticsLabel('All apps'));
    await tester.pump(const Duration(milliseconds: 200));
    _expectNoRadixIcons(tester);
    controller.dispose();
  });

  testWidgets(
    'all history filters use soft borderless select triggers',
    (tester) async {
      const surfaceSize = Size(1200, 800);
      await tester.binding.setSurfaceSize(surfaceSize);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = HistoryController(_TagScreenRepository());
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();

      final selectFinder = find.byWidgetPredicate(
        (widget) => widget is Select<dynamic>,
      );
      expect(selectFinder, findsNWidgets(5));
      expect(find.bySemanticsLabel('All pins'), findsNothing);
      expect(find.bySemanticsLabel('Pinned only'), findsNothing);
      for (final select in tester.widgetList<Select<dynamic>>(selectFinder)) {
        expect(select.filled, isTrue);
        final selectContext = tester.element(find.byWidget(select));
        for (final states in <Set<WidgetState>>[
          const {},
          const {WidgetState.hovered},
          const {WidgetState.focused},
          const {WidgetState.pressed},
        ]) {
          final selectTheme = ComponentTheme.maybeOf<SelectTheme>(
            selectContext,
          )!;
          final decoration =
              selectTheme.decoration!(
                    selectContext,
                    states,
                    const BoxDecoration(),
                  )
                  as BoxDecoration;
          expect(decoration.border?.top.style, BorderStyle.none);
        }
        await tester.tap(find.byWidget(select));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 300));
        final popup = find.byWidgetPredicate((widget) => widget is SelectPopup);
        expect(popup, findsOneWidget);
        final popupRect = tester.getRect(popup);
        expect(popupRect.left, greaterThanOrEqualTo(0));
        expect(popupRect.right, lessThanOrEqualTo(surfaceSize.width));
        expect(popupRect.bottom, lessThanOrEqualTo(surfaceSize.height));
        for (final item in tester.widgetList<SelectItemButton>(
          find.byWidgetPredicate((widget) => widget is SelectItemButton),
        )) {
          final clickable = tester.widget<Clickable>(
            find
                .descendant(
                  of: find.byWidget(item),
                  matching: find.byType(Clickable),
                )
                .first,
          );
          final decoration =
              clickable.decoration!.resolve(const {}) as BoxDecoration;
          expect(
            decoration.border?.top.style ?? BorderStyle.none,
            BorderStyle.none,
          );
        }
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(popup, findsNothing);
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  for (final (platform, size) in [
    (TargetPlatform.macOS, const Size(1400, 900)),
    (TargetPlatform.windows, const Size(1400, 900)),
    (TargetPlatform.android, const Size(320, 568)),
    (TargetPlatform.android, const Size(700, 360)),
  ]) {
    testWidgets(
      'copies from the split button and its select on $platform at $size',
      (tester) async {
        await tester.binding.setSurfaceSize(size);
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final repository = _ScreenRepository()
          ..page = HistoryClipPage(
            items: [
              HistoryClip(
                id: 'copy-clip',
                contentType: 'text/html',
                preview: 'Copy menu clip',
                createdAt: DateTime.utc(2026),
                pinned: false,
              ),
            ],
          );
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light.copyWith(platform: () => platform),
            builder: AppTheme.builder,
            home: Scaffold(child: HistoryScreen(controller: controller)),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('Copy menu clip'));
        await tester.pumpAndSettle();

        final copy = _detailAction('Copy');
        final options = find.byKey(
          const ValueKey<String>('history-copy-options'),
        );
        expect(find.text('Copy plain text'), findsNothing);
        expect(tester.getRect(copy).right, tester.getRect(options).left);
        expect(tester.getSize(copy).height, tester.getSize(options).height);

        await tester.tap(copy);
        await tester.pumpAndSettle();
        expect(repository.copiedIds, ['copy-clip']);
        expect(repository.plainTextCopiedIds, isEmpty);
        await tester.pump(const Duration(seconds: 6));
        await tester.pumpAndSettle();

        final drawerCount = tester
            .widgetList(find.byType(DrawerWrapper))
            .length;
        await tester.tap(options);
        // The select's ContextAnchor polls every frame while the popup is open.
        await tester.pump();
        await tester.pump(AppMotion.standard);
        final popup = find.byType(SelectPopup<bool>);
        expect(popup, findsOneWidget);
        expect(
          OverlayConfiguration.maybeOf(tester.element(popup)),
          isA<PopoverConfiguration>(),
        );
        expect(find.byType(DrawerWrapper), findsNWidgets(drawerCount));
        final popupRect = tester.getRect(popup);
        expect(popupRect.left, greaterThanOrEqualTo(0));
        expect(popupRect.top, greaterThanOrEqualTo(0));
        expect(popupRect.right, lessThanOrEqualTo(size.width));
        expect(popupRect.bottom, lessThanOrEqualTo(size.height));
        expect(popupRect.height, lessThan(AppControlSize.large * 2));
        final plainTextIcon = find.descendant(
          of: popup,
          matching: find.byIcon(LucideIcons.alignLeft),
        );
        expect(
          tester.getRect(find.text('Copy plain text')).left -
              tester.getRect(plainTextIcon).right,
          AppSpacing.sm,
        );
        expect(repository.copiedIds, ['copy-clip']);
        await tester.tap(find.text('Copy plain text'));
        await tester.pumpAndSettle();
        expect(repository.plainTextCopiedIds, ['copy-clip']);
        expect(repository.copiedIds, ['copy-clip']);
        expect(popup, findsNothing);
        expect(tester.takeException(), isNull);
        await tester.pump(const Duration(seconds: 6));
        await tester.pumpAndSettle();
      },
    );
  }

  testWidgets('offers Download for a file whose source is unavailable', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final file = HistoryClip(
      id: 'remote-file',
      contentType: 'file',
      preview: '/Users/person/Documents/report.pdf',
      body: '/Users/person/Documents/report.pdf',
      createdAt: DateTime.utc(2026),
      pinned: false,
      file: const HistoryFileDetails(
        name: 'report.pdf',
        mimeType: 'application/pdf',
        sourceReference: '/Users/person/Documents/report.pdf',
        sourceAvailable: false,
        sizeBytes: 42,
        fileCount: 1,
      ),
    );
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(items: [file]);
    final downloader = _ScreenFileDownloader();
    final pendingSave = Completer<void>();
    final pendingPresentation = Completer<void>();
    repository.savePending = pendingSave.future;
    downloader.presentPending = pendingPresentation.future;
    final controller = HistoryController(
      repository,
      fileDownloader: downloader,
    );
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('/Users/person/Documents/report.pdf').first);
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey<String>('history-copy-options')),
      findsNothing,
    );
    expect(_detailAction('Download'), findsOneWidget);
    await tester.tap(_detailAction('Download'));
    await tester.pump();
    expect(find.text('0%'), findsOneWidget);
    final indicator = find.descendant(
      of: find.byKey(const ValueKey('history-detail-download')),
      matching: find.byType(CircularProgressIndicator),
    );
    final indicatorSize = tester.getSize(indicator);
    expect(indicatorSize.width, greaterThan(0));
    expect(indicatorSize.width, indicatorSize.height);
    expect(
      find.ancestor(of: indicator, matching: find.byType(AspectRatio)),
      findsOneWidget,
    );

    pendingSave.complete();
    await tester.pump();
    await tester.pump();
    expect(find.text('100%'), findsOneWidget);
    pendingPresentation.complete();
    await tester.pumpAndSettle();

    expect(repository.savedFiles, [('remote-file', '/tmp/report.pdf')]);
    expect(downloader.presented, ['/tmp/report.pdf']);
    await tester.pump(const Duration(seconds: 6));
  });
}

Finder _detailAction(String label) {
  if (label == 'Pin') {
    return find.byWidgetPredicate(
      (widget) =>
          widget is Button &&
          widget.key is ValueKey<String> &&
          (widget.key as ValueKey<String>).value.startsWith('history-pin-'),
    );
  }
  return find.byKey(ValueKey<String>('history-detail-${label.toLowerCase()}'));
}

Future<void> _pumpHoverActions(WidgetTester tester) async {
  // Tooltip anchors keep a tracking ticker active, so settle cannot finish.
  await tester.pump();
  await tester.pump(const Duration(seconds: 1));
  await tester.pump(AppMotion.standard);
}

void _expectNoRadixIcons(WidgetTester tester) {
  final radixIcons = tester
      .widgetList<Icon>(find.byType(Icon))
      .where((icon) => icon.icon?.fontFamily == 'RadixIcons');
  expect(radixIcons, isEmpty);
}

class _ScreenRepository implements HistoryRepository {
  @override
  Future<void> importFile(HistoryImportFile file) async {}

  HistoryClipPage page = const HistoryClipPage(items: []);
  HistoryFacets availableFacets = const HistoryFacets();
  final List<(String, String)> savedFiles = [];
  Future<void>? savePending;
  final Map<String, HistorySourceAppIcon> sourceIcons = {};
  final List<String> requestedSourceIconIds = [];
  final List<String> copiedIds = [];
  final List<String> plainTextCopiedIds = [];
  final List<String> deletedIds = [];
  final List<(String, bool)> pinnedUpdates = [];
  bool failPinUpdate = false;
  HistoryImagePreview? availableImagePreview;
  bool fitImagePreviewBounds = false;
  final List<int?> requestedImageEdges = [];
  final List<HistoryImagePreviewBounds?> requestedImageBounds = [];
  final List<HistoryQuery> queries = [];

  @override
  Stream<HistoryRuntimeEvent> watch() =>
      Stream<HistoryRuntimeEvent>.multi((_) {});

  @override
  Future<HistoryFacets> facets() async => availableFacets;

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    queries.add(query);
    return page;
  }

  @override
  Future<HistoryClip> get(String id) async =>
      page.items.firstWhere((clip) => clip.id == id);

  @override
  Future<HistoryImagePreview?> imagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) async {
    requestedImageEdges.add(maxEdge);
    requestedImageBounds.add(bounds);
    final preview = availableImagePreview;
    if (!fitImagePreviewBounds || preview == null || bounds == null) {
      return preview;
    }
    final size = applyBoxFit(
      BoxFit.scaleDown,
      Size(preview.width.toDouble(), preview.height.toDouble()),
      Size(bounds.width.toDouble(), bounds.height.toDouble()),
    ).destination;
    final width = size.width.round().clamp(1, bounds.width);
    final height = size.height.round().clamp(1, bounds.height);
    return HistoryImagePreview(
      raster.encodePng(raster.Image(width: width, height: height)),
      width: width,
      height: height,
    );
  }

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async {
    requestedSourceIconIds.add(id);
    return sourceIcons[id];
  }

  @override
  Future<void> copy(String id) async => copiedIds.add(id);

  @override
  Future<void> copyText(String id, String text) async {}

  @override
  Future<void> copyPlainText(String id) async => plainTextCopiedIds.add(id);

  @override
  Future<void> saveFile(String id, String destinationPath) async {
    savedFiles.add((id, destinationPath));
    await savePending;
  }

  @override
  Future<void> delete(String id) async {
    deletedIds.add(id);
    page = HistoryClipPage(
      items: page.items.where((clip) => clip.id != id).toList(),
    );
  }

  @override
  Future<void> deleteAll() async {}

  @override
  Future<void> reorderPinned(List<String> ids) async {}

  @override
  Future<HistoryClip> setSecret(String id, bool secret) async =>
      throw UnimplementedError('setSecret');

  @override
  Future<void> setPinned(String id, bool pinned) async {
    if (failPinUpdate) throw StateError('Pin update failed');
    pinnedUpdates.add((id, pinned));
    page = HistoryClipPage(
      items: [
        for (final clip in page.items)
          clip.id == id ? clip.copyWith(pinned: pinned) : clip,
      ],
    );
  }
}

class _EditScreenRepository extends _ScreenRepository
    implements HistoryEditRepository {
  @override
  ClipEditRepository get clipEdits => _UnusedSnippets();
  @override
  SnippetRepository get snippets => _UnusedSnippets();
  @override
  ClipEditPreferences get clipEditPreferences => MemoryClipEditPreferences();
}

/// The new clip editor reaches these only on Save, which no test presses.
class _UnusedSnippets implements ClipEditRepository, SnippetRepository {
  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError(invocation.memberName.toString());
}

class _ScreenFileDownloader implements HistoryFileDownloader {
  final List<String> presented = [];
  Future<void>? presentPending;

  @override
  Future<String?> chooseDestination(HistoryFileDetails file) async =>
      '/tmp/report.pdf';

  @override
  Future<void> presentSavedFile(String path, HistoryFileDetails file) async {
    presented.add(path);
    await presentPending;
  }
}

class _CountingScreenRepository extends _ScreenRepository
    implements HistoryGroupCountsRepository {
  HistoryGroupCounts? counts;

  @override
  Future<HistoryGroupCounts?> groupCounts({
    required HistoryQuery query,
    required List<DateTime> days,
  }) async => counts;
}

class _TagScreenRepository extends _ScreenRepository
    implements HistoryTagsRepository {
  @override
  Future<HistoryTags> tags(List<String> clipIds) async =>
      HistoryTags(tags: availableFacets.tags);
  @override
  Future<void> saveTag({String? id, required String name}) =>
      throw UnimplementedError();
  @override
  Future<void> deleteTag(String id) => throw UnimplementedError();
  @override
  Future<void> changeTags({
    required List<String> clipIds,
    required List<String> tagIds,
    required bool assigned,
  }) => throw UnimplementedError();
}

class _RecoveryScreenRepository extends _ScreenRepository {
  bool failAll = true;
  bool rejectFiltered = false;
  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    final page = await super.query(query: query, limit: limit, cursor: cursor);
    if (failAll || (rejectFiltered && query.kind != null)) {
      throw StateError('query refused');
    }
    return page;
  }
}
