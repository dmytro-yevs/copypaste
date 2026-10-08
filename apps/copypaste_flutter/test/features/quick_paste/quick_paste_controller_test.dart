import 'dart:async';

import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:flutter/services.dart';
import 'package:flutter/gestures.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/source_app_label.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/quick_paste/quick_paste_app.dart';
import 'package:copypaste_flutter/features/quick_paste/quick_paste_controller.dart';
import 'package:copypaste_flutter/features/settings/repository/quick_paste_preferences_store.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as image;
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  test('cold engine restores the native inspector state', () async {
    final host = _ContextHost()
      ..openWhenReady = true
      ..initialInspectorVisible = true;
    final controller = QuickPasteController(
      repository: _Repository(),
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    expect(controller.inspectorOpen, isTrue);
    expect(controller.history.selectedClip?.id, 'recent');
  });

  test('cold engine opening waits until its controller is ready', () async {
    final host = _ContextHost()..openWhenReady = true;
    final controller = QuickPasteController(
      repository: _Repository(),
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    expect(host.readyCalls, 1);
    expect(controller.presentationGeneration, 1);
    await controller.activateIndex(0);
    expect(host.closeIds, [1]);
  });

  test(
    'shutdown waits for the owned repository before releasing its engine',
    () async {
      final disposed = Completer<void>();
      var disposals = 0;
      final host = _ContextHost();
      final controller = QuickPasteController(
        repository: _Repository(),
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
        disposeRepository: () {
          disposals += 1;
          return disposed.future;
        },
      );
      await controller.initialize();
      var completed = false;
      final shutdown = host.shutdownHandler!().then((_) => completed = true);
      await Future<void>.delayed(Duration.zero);
      expect(completed, isFalse);
      expect(host.openedHandler, isNull);
      disposed.complete();
      await shutdown;
      await controller.shutdown();
      expect(completed, isTrue);
      expect(disposals, 1);
    },
  );

  testWidgets(
    'loads history pages on scroll and builds only nearby rows',
    (tester) async {
      final repository = _PagedRepository();
      _populateHistory(repository, 125);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
      );
      await tester.binding.setSurfaceSize(const Size(448, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();

      final historyScroll = find.byKey(
        const ValueKey('quick-paste-history-scroll'),
      );
      ScrollController scroll() =>
          tester.widget<ListView>(historyScroll).controller!;
      expect(controller.items, hasLength(50));
      expect(repository.requests, hasLength(1));
      expect(find.byKey(const ValueKey('quick-paste-clip-49')), findsNothing);
      scroll().jumpTo(scroll().position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(100));
      expect(repository.requests.map((request) => request.cursor), [
        null,
        '50',
      ]);
      scroll().jumpTo(scroll().position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(125));
      expect(controller.items.map((clip) => clip.id).toSet(), hasLength(125));
      expect(
        repository.requests.map((request) => request.limit),
        everyElement(50),
      );
      expect(controller.history.canLoadMore, isFalse);
      scroll().jumpTo(scroll().position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey('quick-paste-clip-124')),
        findsOneWidget,
      );
      expect(repository.requests, hasLength(3));

      await controller.search('Clip 1');
      await tester.pumpAndSettle();
      expect(scroll().offset, 0);
      expect(controller.items, hasLength(36));
      expect(repository.requests.last.cursor, isNull);
      expect(repository.requests.last.search, 'Clip 1');
      await controller.opened(1);
      await tester.pumpAndSettle();
      expect(scroll().offset, 0);
      expect(controller.items, hasLength(50));
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  for (final pinned in [false, true]) {
    testWidgets(
      'defers offscreen image previews (pinned: $pinned)',
      (tester) async {
        final repository = _PagedRepository();
        _populateHistory(
          repository,
          80,
          pinnedCount: pinned ? 80 : 0,
          images: true,
        );
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: _ContextHost(),
        );
        await tester.binding.setSurfaceSize(const Size(448, 500));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        await tester.pumpWidget(QuickPasteApp(controller: controller));
        await tester.pumpAndSettle();
        expect(controller.items, hasLength(50));
        expect(repository.imagePreviewIds.length, lessThan(50));
        expect(repository.imagePreviewIds, isNot(contains('clip-49')));
        expect(find.byKey(const ValueKey('quick-paste-clip-49')), findsNothing);
        expect(tester.takeException(), isNull);
      },
      variant: TargetPlatformVariant({
        TargetPlatform.macOS,
        TargetPlatform.windows,
        TargetPlatform.android,
      }),
    );
  }

  testWidgets(
    'paginates a pinned-only page into the recent history',
    (tester) async {
      final repository = _PagedRepository();
      _populateHistory(repository, 125, pinnedCount: 70);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
      );
      await tester.binding.setSurfaceSize(const Size(448, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(50));
      expect(controller.items.every((clip) => clip.pinned), isTrue);
      final pinnedScroll = tester
          .widget<ListView>(
            find.byKey(const ValueKey('quick-paste-pinned-scroll')),
          )
          .controller!;
      pinnedScroll.jumpTo(pinnedScroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(100));
      expect(find.byKey(const ValueKey('quick-paste-clip-70')), findsOneWidget);
      final recentScroll = tester
          .widget<ListView>(
            find.byKey(const ValueKey('quick-paste-history-scroll')),
          )
          .controller!;
      recentScroll.jumpTo(recentScroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(125));
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets(
    'keyboard traverses offscreen rows and page boundaries',
    (tester) async {
      final repository = _PagedRepository();
      _populateHistory(repository, 81, pinnedCount: 1);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
      );
      await tester.binding.setSurfaceSize(const Size(448, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      await controller.opened(1);
      await tester.pumpAndSettle();
      final pinned = find.byKey(const ValueKey('quick-paste-clip-0'));
      final pinnedRect = tester.getRect(pinned);
      for (var page = 0; page < 12; page++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.pageDown);
        await tester.pumpAndSettle();
      }
      expect(controller.focusedClip?.id, 'clip-60');
      expect(controller.items, hasLength(81));
      final focused = find.byKey(const ValueKey('quick-paste-clip-60'));
      expect(focused, findsOneWidget);
      final historyRect = tester.getRect(
        find.byKey(const ValueKey('quick-paste-history-scroll')),
      );
      final focusedRect = tester.getRect(focused);
      expect(focusedRect.top, greaterThanOrEqualTo(historyRect.top));
      expect(focusedRect.bottom, lessThanOrEqualTo(historyRect.bottom));
      expect(tester.getRect(pinned), pinnedRect);
      await tester.sendKeyEvent(LogicalKeyboardKey.pageUp);
      await tester.pumpAndSettle();
      expect(controller.focusedClip?.id, 'clip-55');
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
      await tester.pumpAndSettle();
      expect(controller.focusedClip?.id, 'clip-54');
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('quick-paste-search')))
            .focusNode
            ?.hasFocus,
        isTrue,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(repository.copied, ['clip-54']);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets(
    'failed pages wait for retry and can continue paginating',
    (tester) async {
      final repository = _PagedRepository()..failNextPage = true;
      _populateHistory(repository, 125);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
      );
      await tester.binding.setSurfaceSize(const Size(448, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      final historyScroll = find.byKey(
        const ValueKey('quick-paste-history-scroll'),
      );
      final scroll = tester.widget<ListView>(historyScroll).controller!;
      scroll.jumpTo(scroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(50));
      expect(repository.requests, hasLength(2));
      expect(controller.paginationFailed, isTrue);
      await tester.pumpAndSettle();
      expect(repository.requests, hasLength(2));
      await tester.tap(find.text('Try again'));
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(100));
      expect(controller.paginationFailed, isFalse);
      scroll.jumpTo(scroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(125));
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  test('scroll and keyboard share the pending page request', () async {
    final repository = _PagedRepository();
    _populateHistory(repository, 125);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.focus(controller.items.last);
    repository.pageGate = Completer<void>();
    final page = controller.loadMore();
    await repository.pageStarted.future;
    final navigation = controller.moveFocus(1);
    expect(repository.requests, hasLength(2));
    repository.pageGate!.complete();
    await Future.wait([page, navigation]);
    expect(repository.requests, hasLength(2));
    expect(controller.items, hasLength(100));
    expect(controller.focusedClip?.id, 'clip-50');
  });

  test('page navigation loads only the final inspector selection', () async {
    final repository = _PagedRepository();
    _populateHistory(repository, 125);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1, inspectorVisible: true);
    expect(repository.getCalls, 1);
    await controller.moveFocus(5);
    expect(controller.focusedClip?.id, 'clip-4');
    expect(controller.history.selectedClip?.id, 'clip-4');
    expect(repository.getCalls, 2);
  });

  test('search cancels pending keyboard pagination', () async {
    final repository = _PagedRepository();
    _populateHistory(repository, 125);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.focus(controller.items.last);
    repository.pageGate = Completer<void>();
    final navigation = controller.moveFocus(1);
    await repository.pageStarted.future;
    await controller.search('Clip 124');
    repository.pageGate!.complete();
    await navigation;
    expect(controller.items.map((clip) => clip.id), ['clip-124']);
    expect(controller.focusedClip, isNull);
    expect(controller.paginationFailed, isFalse);
  });

  testWidgets('renders the keyboard-first popup and footer actions', (
    tester,
  ) async {
    final controller = QuickPasteController(
      repository: _Repository(),
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );

    await tester.binding.setSurfaceSize(const Size(448, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle(const Duration(milliseconds: 150));

    expect(find.text('Type to search…'), findsOneWidget);
    expect(find.text('Pinned clip'), findsOneWidget);
    expect(find.text('Recent clip'), findsOneWidget);
    final sourceApps = find.byType(SourceAppLabel);
    expect(sourceApps, findsNWidgets(2));
    expect(
      tester.widgetList<SourceAppLabel>(sourceApps).map((label) => label.name),
      everyElement('Editor'),
    );
    expect(find.text('Clear'), findsOneWidget);
    expect(find.text('Preferences…'), findsOneWidget);
    expect(find.text('About'), findsOneWidget);
    expect(find.text('Quit'), findsOneWidget);
    expect(find.byType(Command), findsNothing);
    expect(find.byType(Alert), findsNothing);
    expect(find.text('CopyPaste'), findsNothing);
    expect(
      tester.getSize(find.byKey(const ValueKey('quick-paste-logo'))),
      const Size(16, 16),
    );
    final logo = tester.getRect(find.byKey(const ValueKey('quick-paste-logo')));
    final search = tester.getRect(
      find.byKey(const ValueKey('quick-paste-search')),
    );
    final inspectorToggle = tester.getRect(
      find.byKey(const ValueKey('quick-paste-inspector-toggle')),
    );
    expect(logo.center.dy, closeTo(search.center.dy, 0.1));
    expect(logo.center.dy, closeTo(inspectorToggle.center.dy, 0.1));
    expect(search.left - logo.right, 4);
    expect(
      tester.getTopLeft(find.text('Pinned clip')).dy,
      greaterThan(tester.getTopLeft(find.text('Recent clip')).dy),
    );
    expect(
      tester.getTopLeft(find.text('Preferences…')).dy,
      greaterThan(tester.getTopLeft(find.text('Clear')).dy),
    );
    expect(
      tester
          .widgetList<SourceAppLabel>(sourceApps)
          .map((label) => label.showName),
      everyElement(false),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('keeps pinned clips fixed below scrolling history', (
    tester,
  ) async {
    final repository = _Repository();
    repository.clips.addAll([
      for (var index = 0; index < 40; index++)
        HistoryClip(
          id: 'recent-$index',
          contentType: 'text/plain',
          preview: 'Recent clip $index',
          createdAt: DateTime.utc(2026),
          pinned: false,
        ),
    ]);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    await tester.binding.setSurfaceSize(const Size(448, 500));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();

    final historyScroll = find.byKey(
      const ValueKey('quick-paste-history-scroll'),
    );
    final pinned = find.byKey(const ValueKey('quick-paste-pinned'));
    final pinnedRect = tester.getRect(pinned);
    final footerRect = tester.getRect(find.text('Clear'));
    expect(find.descendant(of: historyScroll, matching: pinned), findsNothing);
    expect(
      pinnedRect.top,
      greaterThanOrEqualTo(tester.getRect(historyScroll).bottom),
    );
    expect(pinnedRect.bottom, lessThan(footerRect.top));

    await tester.drag(historyScroll, const Offset(0, -300));
    await tester.pumpAndSettle();
    final scroll = tester.widget<ListView>(historyScroll);
    expect(scroll.controller!.offset, greaterThan(0));
    expect(tester.getRect(pinned), pinnedRect);
    expect(tester.getRect(find.text('Clear')), footerRect);
    expect(tester.takeException(), isNull);
  });

  for (final recentCount in [0, 40]) {
    testWidgets('fits many pinned clips with $recentCount recent clips', (
      tester,
    ) async {
      final repository = _Repository()..clips.clear();
      repository.clips.addAll([
        for (var index = 0; index < 40 + recentCount; index++)
          HistoryClip(
            id: 'clip-$index',
            contentType: 'text/plain',
            preview: 'Clip $index',
            createdAt: DateTime.utc(2026),
            pinned: index < 40,
          ),
      ]);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost()..permissionGranted = true,
      );
      await tester.binding.setSurfaceSize(const Size(448, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      final pinnedScroll = find.byKey(
        const ValueKey('quick-paste-pinned-scroll'),
      );
      final footerRect = tester.getRect(find.text('Clear'));
      await tester.drag(pinnedScroll, const Offset(0, -300));
      await tester.pumpAndSettle();
      expect(tester.getRect(find.text('Clear')), footerRect);
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets(
    'keeps source icons stable across full-page rebuilds',
    (tester) async {
      final repository = _Repository()..clips.clear();
      repository.sourceIcon = HistorySourceAppIcon(
        Uint8List.fromList(image.encodePng(image.Image(width: 16, height: 16))),
      );
      repository.clips.addAll([
        for (var index = 0; index < 50; index++)
          HistoryClip(
            id: 'icon-$index',
            contentType: 'text/plain',
            preview: 'Clip $index',
            createdAt: DateTime.utc(2026),
            pinned: false,
            sourceApp: 'Editor',
            sourceAppIconId: 'app:editor',
          ),
      ]);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost()..permissionGranted = true,
      );
      await tester.binding.setSurfaceSize(const Size(448, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      for (var frame = 0; frame < 12; frame++) {
        await tester.pump(const Duration(milliseconds: 100));
      }

      expect(repository.sourceIconCalls, 1);
      final iconFinder = find.descendant(
        of: find.byKey(const ValueKey('quick-paste-icon-0')),
        matching: find.byType(Avatar),
      );
      final provider = tester.widget<Avatar>(iconFinder).provider;
      expect(provider, isNotNull);
      controller.focus(controller.items[1]);
      for (var frame = 0; frame < 4; frame++) {
        await tester.pump(const Duration(milliseconds: 100));
        expect(tester.widget<Avatar>(iconFinder).provider, provider);
      }
      expect(repository.sourceIconCalls, 1);
      await controller.history.deleteClip('icon-0');
      await tester.pump(const Duration(milliseconds: 100));
      final nextIconFinder = find.descendant(
        of: find.byKey(const ValueKey('quick-paste-icon-1')),
        matching: find.byType(Avatar),
      );
      expect(tester.widget<Avatar>(nextIconFinder).provider, provider);
      expect(repository.sourceIconCalls, 1);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets('image rows show large content previews without image labels', (
    tester,
  ) async {
    final repository = _Repository();
    repository.clips.insert(
      0,
      HistoryClip(
        id: 'image',
        contentType: 'image/png',
        preview: '[image]',
        createdAt: DateTime.utc(2026),
        pinned: false,
        sourceApp: 'Editor',
        sourceAppIconId: 'app:editor',
      ),
    );
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    await tester.binding.setSurfaceSize(const Size(448, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    final preview = find.descendant(
      of: find.byKey(const ValueKey('quick-paste-image')),
      matching: find.byType(Image),
    );
    expect(find.text('[image]'), findsNothing);
    expect(tester.getSize(preview).height, 120);
    expect(repository.imagePreviewEdges, contains(480));
    expect(tester.takeException(), isNull);
  });

  testWidgets('arrows select the visible menu order while search keeps focus', (
    tester,
  ) async {
    final repository = _Repository();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    await tester.binding.setSurfaceSize(const Size(448, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    await controller.opened(1);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(controller.focusedClip?.id, 'recent');
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(controller.focusedClip?.id, 'pinned');
    expect(
      tester
          .widget<TextField>(find.byKey(const ValueKey('quick-paste-search')))
          .focusNode
          ?.hasFocus,
      isTrue,
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(repository.copied, ['pinned']);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'hover opens the inspector after the Maccy delay and follows selection',
    (tester) async {
      final repository = _Repository();
      final host = _ContextHost()..openWhenReady = true;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      await tester.binding.setSurfaceSize(const Size(816, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      addTearDown(mouse.removePointer);
      final recent = find.byKey(const ValueKey('quick-paste-recent'));
      final pinned = find.byKey(const ValueKey('quick-paste-pinned'));
      await mouse.moveTo(tester.getCenter(recent));
      await tester.pump(const Duration(milliseconds: 1499));
      expect(controller.focusedClip?.id, 'recent');
      expect(controller.inspectorOpen, isFalse);
      expect(repository.getCalls, 0);
      await tester.pump(const Duration(milliseconds: 1));
      await tester.pumpAndSettle();
      expect(host.inspectorChanges, [true]);
      expect(host.inspectorPresentationIds, [1]);
      expect(controller.inspectorOpen, isTrue);
      expect(controller.history.selectedClip?.id, 'recent');
      expect(
        find.byKey(const ValueKey('history-detail-inspector')),
        findsOneWidget,
      );
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('quick-paste-search')))
            .focusNode
            ?.hasFocus,
        isTrue,
      );
      await mouse.moveTo(tester.getCenter(pinned));
      await tester.pumpAndSettle();
      expect(controller.history.selectedClip?.id, 'pinned');
      expect(host.inspectorChanges, [true]);
      await mouse.moveTo(
        tester.getCenter(
          find.byKey(const ValueKey('history-detail-inspector')),
        ),
      );
      await tester.pump(const Duration(seconds: 2));
      expect(controller.inspectorOpen, isTrue);
      expect(repository.copied, isEmpty);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets('leaving and changing hovered rows cancel the previous delay', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost()..openWhenReady = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    await tester.binding.setSurfaceSize(const Size(816, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);
    final recent = find.byKey(const ValueKey('quick-paste-recent'));
    final pinned = find.byKey(const ValueKey('quick-paste-pinned'));
    await mouse.moveTo(tester.getCenter(recent));
    await tester.pump(const Duration(milliseconds: 1000));
    await mouse.moveTo(
      tester.getCenter(find.byKey(const ValueKey('quick-paste-search'))),
    );
    await tester.pump(const Duration(seconds: 2));
    expect(controller.inspectorOpen, isFalse);
    expect(host.inspectorChanges, isEmpty);
    await mouse.moveTo(tester.getCenter(recent));
    await tester.pump(const Duration(milliseconds: 1000));
    await mouse.moveTo(tester.getCenter(pinned));
    await tester.pump(const Duration(milliseconds: 1000));
    expect(controller.inspectorOpen, isFalse);
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pumpAndSettle();
    expect(controller.history.selectedClip?.id, 'pinned');
    expect(repository.getCalls, 1);
    expect(host.inspectorChanges, [true]);
    expect(tester.takeException(), isNull);
  });

  testWidgets('scrolling cancels hover on a row that leaves the lazy list', (
    tester,
  ) async {
    final repository = _PagedRepository();
    _populateHistory(repository, 125);
    final host = _ContextHost()..openWhenReady = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    await tester.binding.setSurfaceSize(const Size(816, 500));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);
    await mouse.moveTo(
      tester.getCenter(find.byKey(const ValueKey('quick-paste-clip-0'))),
    );
    await tester.pump(const Duration(milliseconds: 1000));
    final scroll = tester
        .widget<ListView>(
          find.byKey(const ValueKey('quick-paste-history-scroll')),
        )
        .controller!;
    scroll.jumpTo(scroll.position.maxScrollExtent);
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.byKey(const ValueKey('quick-paste-clip-0')), findsNothing);
    expect(controller.inspectorOpen, isFalse);
    expect(host.inspectorChanges, isEmpty);
    await mouse.moveTo(Offset.zero);
    await tester.pump(const Duration(seconds: 2));
    expect(host.inspectorChanges, isEmpty);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'manual close suppresses the hovered clip until selection changes',
    (tester) async {
      final repository = _Repository();
      final host = _ContextHost();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.opened(1);
      final recent = repository.clips.last;
      controller.hoverClip(recent, hovered: true);
      await tester.pump(const Duration(milliseconds: 1500));
      expect(controller.inspectorOpen, isTrue);
      await controller.toggleInspector();
      controller.hoverClip(recent, hovered: false);
      controller.hoverClip(recent, hovered: true);
      await tester.pump(const Duration(seconds: 2));
      expect(controller.inspectorOpen, isFalse);
      expect(host.inspectorChanges, [true, false]);
      controller.hoverClip(repository.clips.first, hovered: true);
      await tester.pump(const Duration(milliseconds: 1500));
      expect(controller.inspectorOpen, isTrue);
      expect(controller.history.selectedClip?.id, 'pinned');
      expect(host.inspectorChanges, [true, false, true]);
    },
  );

  for (final action in [
    'search',
    'keyboard',
    'close',
    'reopen',
    'paste',
    'main',
    'settings',
    'quit',
    'shutdown',
    'delete',
  ]) {
    testWidgets('$action cancels a pending hover inspector', (tester) async {
      final repository = _Repository();
      final host = _ContextHost();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.opened(1);
      final recent = repository.clips.last;
      controller.hoverClip(recent, hovered: true);
      await tester.pump(const Duration(milliseconds: 1000));
      switch (action) {
        case 'search':
          await controller.search('new');
        case 'keyboard':
          await controller.moveFocus(1);
        case 'close':
          await controller.close();
        case 'reopen':
          await controller.opened(2);
        case 'paste':
          await controller.activate(recent);
        case 'main':
          await controller.openMainWindow();
        case 'settings':
          await controller.openSettings();
        case 'quit':
          await controller.quit();
        case 'shutdown':
          await controller.shutdown();
        case 'delete':
          await controller.history.deleteClip(recent.id);
        default:
          throw StateError('Unknown cancellation action: $action');
      }
      await tester.pump(const Duration(seconds: 2));
      expect(host.inspectorChanges, isEmpty);
    });
  }

  testWidgets('manual close wins over a pending automatic native open', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost()..inspectorGate = Completer<void>();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    controller.hoverClip(repository.clips.last, hovered: true);
    await tester.pump(const Duration(milliseconds: 1500));
    expect(host.inspectorChanges, [true]);
    final close = controller.toggleInspector();
    host.inspectorGate!.complete();
    await tester.pump();
    await close;
    expect(controller.inspectorOpen, isFalse);
    expect(host.inspectorChanges, [true, false]);
    expect(host.inspectorPresentationIds, [1, 1]);
    await tester.pump(const Duration(seconds: 2));
    expect(host.inspectorChanges, [true, false]);
  });

  testWidgets('hovering another clip can reopen after a pending manual close', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    controller.hoverClip(repository.clips.last, hovered: true);
    await tester.pump(const Duration(milliseconds: 1500));
    host.inspectorGate = Completer<void>();
    final close = controller.toggleInspector();
    await tester.pump();
    controller.hoverClip(repository.clips.first, hovered: true);
    await tester.pump(const Duration(milliseconds: 1500));
    expect(host.inspectorChanges, [true, false]);
    host.inspectorGate!.complete();
    await tester.pump();
    await close;
    expect(host.inspectorChanges, [true, false, true]);
    expect(controller.inspectorOpen, isTrue);
    expect(controller.history.selectedClip?.id, 'pinned');
  });

  testWidgets('a reopened popup ignores completion of an old hover open', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost()..inspectorGate = Completer<void>();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    controller.hoverClip(repository.clips.last, hovered: true);
    await tester.pump(const Duration(milliseconds: 1500));
    await controller.opened(2);
    host.inspectorGate!.complete();
    await tester.pump();
    expect(controller.inspectorOpen, isFalse);
    expect(repository.getCalls, 0);
    expect(host.inspectorPresentationIds, [1]);
  });

  testWidgets('slow inspector content never blocks manual closing', (
    tester,
  ) async {
    final repository = _Repository()..selectGate = Completer<HistoryClip>();
    final host = _ContextHost();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    controller.hoverClip(repository.clips.last, hovered: true);
    await tester.pump(const Duration(milliseconds: 1500));
    expect(controller.inspectorOpen, isTrue);
    await controller.toggleInspector();
    expect(controller.inspectorOpen, isFalse);
    expect(host.inspectorChanges, [true, false]);
    repository.selectGate!.complete(repository.clips.last);
    await tester.pump();
    expect(controller.inspectorOpen, isFalse);
  });

  for (final failure in [
    PlatformException(code: 'window_unavailable'),
    MissingPluginException(),
  ]) {
    testWidgets('hover tolerates an unavailable native inspector: $failure', (
      tester,
    ) async {
      final repository = _Repository();
      final host = _ContextHost()..inspectorFailure = failure;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.opened(1);
      controller.hoverClip(repository.clips.last, hovered: true);
      await tester.pump(const Duration(milliseconds: 1500));
      expect(controller.inspectorOpen, isFalse);
      expect(repository.getCalls, 0);
      expect(tester.takeException(), isNull);
    });
  }

  test('number shortcuts activate the visible menu order', () async {
    final repository = _Repository();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    await controller.activateIndex(0);
    expect(repository.copied, ['recent']);
    await controller.opened(2);
    await controller.activateIndex(1);
    expect(repository.copied, ['recent']);
  });

  testWidgets('inspector shares History content and follows the focused clip', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    await tester.binding.setSurfaceSize(const Size(816, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    await controller.opened(1);
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('quick-paste-inspector-toggle')),
    );
    await tester.pumpAndSettle();
    expect(host.inspectorChanges, [true]);
    expect(controller.history.selectedClip?.id, 'recent');
    expect(
      find.byKey(const ValueKey('history-detail-inspector')),
      findsOneWidget,
    );
    controller.focus(repository.clips.first);
    await tester.pumpAndSettle();
    expect(controller.history.selectedClip?.id, 'pinned');
    await tester.tap(
      find.byKey(const ValueKey('history-detail-inspector-close')),
    );
    await tester.pumpAndSettle();
    expect(host.inspectorChanges, [true, false]);
    expect(
      find.byKey(const ValueKey('history-detail-inspector')),
      findsNothing,
    );
    expect(tester.takeException(), isNull);
  });

  test(
    'pinned letter shortcuts remain stable across filtering and restart',
    () async {
      final repository = _Repository();
      final store = MemoryQuickPastePreferencesStore();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: store,
        host: _ContextHost()..permissionGranted = true,
      );
      await controller.initialize();
      final pin = repository.clips.first;
      final key = controller.shortcutFor(pin);
      expect(key, isNotNull);
      expect(key, isNot(LogicalKeyboardKey.digit2));
      expect(controller.shortcuts[LogicalKeyboardKey.digit1]?.id, 'recent');
      expect(controller.shortcuts[key]?.id, 'pinned');
      await controller.opened(1);
      await controller.activate(controller.shortcuts[key]!);
      expect(repository.copied, ['pinned']);
      repository.clips.remove(pin);
      await controller.search('recent');
      repository.clips.add(pin);
      await controller.opened(2);
      expect(controller.shortcutFor(pin), key);
      controller.dispose();
      final restarted = QuickPasteController(
        repository: repository,
        preferencesStore: store,
        host: _ContextHost()..permissionGranted = true,
      );
      addTearDown(restarted.dispose);
      await restarted.initialize();
      expect(restarted.shortcutFor(pin), key);
    },
  );

  test(
    'pinned shortcuts are unique and exclude system and menu actions',
    () async {
      final repository = _Repository();
      repository.clips.add(
        HistoryClip(
          id: 'second-pin',
          contentType: 'text/plain',
          preview: 'Second pin',
          createdAt: DateTime.utc(2026),
          pinned: true,
        ),
      );
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost()..permissionGranted = true,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      final pinned = repository.clips.where((clip) => clip.pinned).toList();
      final keys = pinned.map(controller.shortcutFor).toSet();
      expect(keys.length, 2);
      expect(keys, isNot(contains(null)));
      for (final reserved in [
        LogicalKeyboardKey.keyA,
        LogicalKeyboardKey.keyV,
        LogicalKeyboardKey.keyQ,
        LogicalKeyboardKey.keyW,
        LogicalKeyboardKey.keyZ,
        LogicalKeyboardKey.keyP,
      ]) {
        expect(keys, isNot(contains(reserved)));
      }
    },
  );

  test(
    'Accessibility is requested only for paste and a denial survives reopen',
    () async {
      final store = MemoryQuickPastePreferencesStore();
      final repository = _Repository();
      final host = _ContextHost();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: store,
        host: host,
      );
      await controller.initialize();
      await controller.opened(1);
      await controller.opened(2);
      expect(host.permissionRequests, 0);
      await controller.activateIndex(0);
      expect(host.permissionRequests, 1);
      await controller.opened(3);
      await controller.activateIndex(0);
      expect(host.permissionRequests, 1);
      controller.dispose();

      final restarted = QuickPasteController(
        repository: repository,
        preferencesStore: store,
        host: host,
      );
      addTearDown(restarted.dispose);
      await restarted.initialize();
      await restarted.opened(4);
      await restarted.activateIndex(0);
      expect(host.permissionRequests, 1);
      expect(repository.copied, ['recent', 'recent', 'recent']);
      expect(host.pasteCalls, 0);
    },
  );

  test('copy-only selection never requests Accessibility', () async {
    final host = _ContextHost();
    final controller = QuickPasteController(
      repository: _Repository(),
      preferencesStore: MemoryQuickPastePreferencesStore(
        QuickPastePreferences(
          autoPaste: false,
          shortcut: DesktopShortcut.defaultForPlatform(),
        ),
      ),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    await controller.activateIndex(0);
    expect(host.permissionRequests, 0);
    expect(host.closeCalls, 1);
  });

  test('auto-pastes the copied clip when permission is available', () async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);

    final clip = controller.history.items.last;
    controller.focus(clip);
    await controller.activateFocused();

    expect(repository.copied, ['recent']);
    expect(host.pasteCalls, 1);
    expect(host.closeCalls, 0);
  });

  test('falls back to copy and close when Accessibility is denied', () async {
    final repository = _Repository();
    final host = _ContextHost();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);

    await controller.activate(controller.history.items.last);

    expect(host.permissionRequests, 1);
    expect(repository.copied, ['recent']);
    expect(host.pasteCalls, 0);
    expect(host.closeCalls, 1);
  });

  test('Option selection inverts copy-only mode for one paste', () async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(
        QuickPastePreferences(
          autoPaste: false,
          shortcut: DesktopShortcut.defaultForPlatform(),
        ),
      ),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);

    await controller.activate(
      controller.history.items.last,
      invertAutoPaste: true,
    );

    expect(host.pasteCalls, 1);
    expect(host.closeCalls, 0);
  });

  test('clear keeps pinned clips visible', () async {
    final repository = _Repository();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);

    expect(await controller.clearUnpinned(), isTrue);
    expect(controller.history.items.map((clip) => clip.id), ['pinned']);
    expect(repository.deleteAllCalls, 1);
  });
  test(
    'reopen during Copy keeps one owner and never queues another write',
    () async {
      final repository = _Repository();
      final host = _ContextHost()..permissionGranted = true;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.opened(1);
      repository.copyGate = Completer<void>();
      final oldAction = controller.activate(controller.history.items.last);
      await repository.copyStarted.future;
      await controller.opened(2);
      await controller.activate(controller.history.items.first);
      expect(controller.activating, isTrue);
      expect(repository.copied, ['recent']);
      repository.copyGate!.complete();
      await oldAction;
      expect(controller.activating, isFalse);
      expect(host.pasteIds, isEmpty);
      expect(host.closeIds, isEmpty);
      expect(repository.copied, ['recent']);

      repository.copyGate = Completer<void>();
      repository.copyStarted = Completer<void>();
      final newAction = controller.activate(controller.history.items.first);
      await repository.copyStarted.future;
      expect(controller.activating, isTrue);
      repository.copyGate!.complete();
      await newAction;
      expect(controller.activating, isFalse);
      expect(repository.copied, ['recent', 'pinned']);
      expect(host.pasteIds, [2]);
    },
  );

  test('reopen while selection is pending starts no stale Copy', () async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    repository.selectGate = Completer<HistoryClip>();
    final clip = controller.history.items.last;
    final action = controller.activate(clip);
    await repository.selectStarted.future;
    await controller.opened(2);
    expect(controller.activating, isTrue);
    await controller.activate(controller.history.items.first);
    repository.selectGate!.complete(clip);
    await action;
    expect(repository.copied, isEmpty);
    expect(host.pasteIds, isEmpty);
    expect(host.closeIds, isEmpty);
  });

  test(
    'native reopen before Dart opened rejects old paste and conditional close',
    () async {
      final repository = _Repository();
      final host = _ContextHost()..permissionGranted = true;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.opened(1);
      repository.copyGate = Completer<void>();
      final action = controller.activate(controller.history.items.last);
      await repository.copyStarted.future;
      host.nativeId = 2;
      repository.copyGate!.complete();
      await action;
      expect(repository.copied, ['recent']);
      expect(host.pasteIds, [1]);
      expect(host.closeIds, [1]);
      expect(host.actualCloseIds, isEmpty);
      expect(host.nativeId, 2);
    },
  );

  for (final failure in [
    false,
    PlatformException(code: 'gone'),
    MissingPluginException(),
  ]) {
    test(
      'optional paste failure $failure preserves one Copy and closes its ID',
      () async {
        final repository = _Repository();
        final host = _ContextHost()..permissionGranted = true;
        if (failure is bool) {
          host.pasteResult = failure;
        } else {
          host.pasteFailure = failure;
        }
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
        );
        addTearDown(controller.dispose);
        await controller.initialize();
        await controller.opened(1);
        await controller.activate(controller.history.items.last);
        expect(repository.copied, ['recent']);
        expect(host.pasteIds, [1]);
        expect(host.closeIds, [1]);
        expect(controller.activating, isFalse);
      },
    );
  }

  test(
    'old pending paste false cannot close a newer Dart presentation',
    () async {
      final repository = _Repository();
      final host = _ContextHost()..permissionGranted = true;
      host.pasteGate = Completer<bool>();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.opened(1);
      final action = controller.activate(controller.history.items.last);
      await host.pasteStarted.future;
      await controller.opened(2);
      await controller.activate(controller.history.items.first);
      expect(controller.activating, isTrue);
      host.pasteGate!.complete(false);
      await action;
      expect(repository.copied, ['recent']);
      expect(host.closeIds, isEmpty);
    },
  );

  for (final copyFails in [false, true]) {
    test(
      'disposal during Copy suppresses late host calls and notifications ($copyFails)',
      () async {
        final repository = _Repository();
        final host = _ContextHost()..permissionGranted = true;
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
        );
        await controller.initialize();
        await controller.opened(1);
        repository.copyGate = Completer<void>();
        final action = controller.activate(controller.history.items.last);
        await repository.copyStarted.future;
        var notifications = 0;
        controller.addListener(() {
          notifications++;
        });
        controller.dispose();
        if (copyFails) {
          repository.copyGate!.completeError(StateError('copy failed'));
        } else {
          repository.copyGate!.complete();
        }
        await action;
        expect(notifications, 0);
        expect(host.pasteIds, isEmpty);
        expect(host.closeIds, isEmpty);
        expect(host.openedHandler, isNull);
        expect(controller.activating, isFalse);
      },
    );
  }

  test(
    'disposal while selection is pending starts no Copy or host action',
    () async {
      final repository = _Repository();
      final host = _ContextHost()..permissionGranted = true;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      await controller.initialize();
      await controller.opened(1);
      repository.selectGate = Completer<HistoryClip>();
      final clip = controller.history.items.last;
      final action = controller.activate(clip);
      await repository.selectStarted.future;
      controller.dispose();
      repository.selectGate!.complete(clip);
      await action;
      expect(repository.copied, isEmpty);
      expect(host.pasteIds, isEmpty);
      expect(host.closeIds, isEmpty);
    },
  );

  test('copy-only mode closes after one Copy without native paste', () async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(
        QuickPastePreferences(
          autoPaste: false,
          shortcut: DesktopShortcut.defaultForPlatform(),
        ),
      ),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    await controller.activate(controller.history.items.last);
    expect(repository.copied, ['recent']);
    expect(host.pasteIds, isEmpty);
    expect(host.closeIds, [1]);
    await controller.opened(2);
    await controller.activate(controller.history.items.first, forcePaste: true);
    expect(host.pasteIds, [2]);
  });
}

class _Repository implements HistoryRepository {
  @override
  Future<void> importFile(HistoryImportFile file) async {}

  HistorySourceAppIcon? sourceIcon;
  int sourceIconCalls = 0;
  final clips = <HistoryClip>[
    HistoryClip(
      id: 'pinned',
      contentType: 'text/plain',
      preview: 'Pinned clip',
      createdAt: DateTime.utc(2026),
      pinned: true,
      sourceApp: 'Editor',
      sourceAppIconId: 'app:editor',
    ),
    HistoryClip(
      id: 'recent',
      contentType: 'text/plain',
      preview: 'Recent clip',
      createdAt: DateTime.utc(2026),
      pinned: false,
      sourceApp: 'Editor',
      sourceAppIconId: 'app:editor',
    ),
  ];
  final copied = <String>[];
  final imagePreviewEdges = <int?>[];
  final imagePreviewIds = <String>[];
  int deleteAllCalls = 0;
  int getCalls = 0;
  Completer<void>? copyGate;
  Completer<void> copyStarted = Completer<void>();
  Completer<HistoryClip>? selectGate;
  Completer<void> selectStarted = Completer<void>();

  @override
  Future<void> copy(String id) async {
    copied.add(id);
    if (!copyStarted.isCompleted) copyStarted.complete();
    await copyGate?.future;
  }

  @override
  Future<void> copyPlainText(String id) => copy(id);

  @override
  Future<void> saveFile(String id, String destinationPath) async {}

  @override
  Future<void> delete(String id) async {
    clips.removeWhere((clip) => clip.id == id);
  }

  @override
  Future<void> deleteAll() async {
    deleteAllCalls += 1;
    clips.removeWhere((clip) => !clip.pinned);
  }

  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();

  @override
  Future<HistoryClip> get(String id) async {
    getCalls += 1;
    if (!selectStarted.isCompleted) selectStarted.complete();
    return await selectGate?.future ??
        clips.singleWhere((clip) => clip.id == id);
  }

  @override
  Future<HistoryImagePreview?> imagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) async {
    imagePreviewEdges.add(maxEdge);
    imagePreviewIds.add(id);
    if (id != 'image') return null;
    final preview = image.Image(width: 240, height: 180);
    return HistoryImagePreview(
      Uint8List.fromList(image.encodePng(preview)),
      width: 240,
      height: 180,
    );
  }

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async => HistoryClipPage(items: List.of(clips));

  @override
  Future<void> reorderPinned(List<String> ids) async {}

  @override
  Future<void> setPinned(String id, bool pinned) async {}

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async {
    sourceIconCalls += 1;
    return sourceIcon == null
        ? null
        : HistorySourceAppIcon(Uint8List.fromList(sourceIcon!.bytes));
  }

  @override
  Stream<HistoryRuntimeEvent> watch() => const Stream.empty();
}

void _populateHistory(
  _Repository repository,
  int count, {
  int pinnedCount = 0,
  bool images = false,
}) {
  repository.clips
    ..clear()
    ..addAll([
      for (var index = 0; index < count; index++)
        HistoryClip(
          id: 'clip-$index',
          contentType: images ? 'image/png' : 'text/plain',
          preview: 'Clip $index',
          createdAt: DateTime.utc(2026),
          pinned: index < pinnedCount,
        ),
    ]);
}

class _PagedRepository extends _Repository {
  final requests = <({int limit, String? cursor, String search})>[];
  bool failNextPage = false;
  Completer<void>? pageGate;
  final pageStarted = Completer<void>();

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    requests.add((limit: limit, cursor: cursor, search: query.search));
    if (cursor != null) {
      if (!pageStarted.isCompleted) pageStarted.complete();
      await pageGate?.future;
      if (failNextPage) {
        failNextPage = false;
        throw StateError('Page unavailable');
      }
    }
    final matches = clips.where((clip) => clip.preview.contains(query.search));
    final ordered = [
      ...matches.where((clip) => clip.pinned),
      ...matches.where((clip) => !clip.pinned),
    ];
    final offset = cursor == null ? 0 : int.parse(cursor);
    final page = ordered.skip(offset).take(limit).toList();
    final next = offset + page.length;
    return HistoryClipPage(
      items: page,
      nextCursor: next < ordered.length ? '$next' : null,
    );
  }
}

class _ContextHost implements QuickPasteContextHost {
  @override
  void setShutdownHandler(Future<void> Function()? handler) {
    shutdownHandler = handler;
  }

  @override
  Future<void> signalReady() async {
    readyCalls += 1;
    if (openWhenReady) {
      await openedHandler?.call(1, inspectorVisible: initialInspectorVisible);
    }
  }

  Future<void> Function()? shutdownHandler;
  int readyCalls = 0;
  bool openWhenReady = false;
  bool initialInspectorVisible = false;
  final inspectorChanges = <bool>[];
  final inspectorPresentationIds = <int>[];
  Completer<void>? inspectorGate;
  Object? inspectorFailure;

  @override
  Future<void> setInspectorVisible({
    required int presentationId,
    required bool visible,
  }) async {
    inspectorChanges.add(visible);
    inspectorPresentationIds.add(presentationId);
    final failure = inspectorFailure;
    if (failure != null) throw failure;
    await inspectorGate?.future;
  }

  bool permissionGranted = false;
  int permissionRequests = 0;
  int pasteCalls = 0;
  int closeCalls = 0;
  QuickPasteOpenedHandler? openedHandler;
  final pasteIds = <int>[];
  final closeIds = <int>[];
  final actualCloseIds = <int>[];
  int? nativeId;
  bool pasteResult = true;
  Object? pasteFailure;
  Completer<bool>? pasteGate;
  Completer<void> pasteStarted = Completer<void>();

  @override
  Future<bool> accessibilityGranted() async => permissionGranted;

  @override
  Future<void> close({required int presentationId}) async {
    closeCalls += 1;
    closeIds.add(presentationId);
    if (nativeId == null || nativeId == presentationId) {
      actualCloseIds.add(presentationId);
    }
  }

  @override
  Future<void> dispose() async {}

  @override
  Future<void> openMainWindow() async {}

  @override
  Future<void> openSettings() async {}

  @override
  Future<bool> paste({required int presentationId}) async {
    pasteCalls += 1;
    pasteIds.add(presentationId);
    if (!pasteStarted.isCompleted) pasteStarted.complete();
    if (pasteFailure case final Object error) throw error;
    if (nativeId != null && nativeId != presentationId) return false;
    return await pasteGate?.future ?? pasteResult;
  }

  @override
  Future<void> quit() async {}

  @override
  Future<bool> requestAccessibility() async {
    permissionRequests += 1;
    return permissionGranted;
  }

  @override
  void setOpenedHandler(QuickPasteOpenedHandler? handler) {
    openedHandler = handler;
  }
}
