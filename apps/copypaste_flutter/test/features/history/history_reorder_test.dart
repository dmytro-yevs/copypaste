import 'dart:async';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  test(
    'saves a loaded prefix, preserves unseen pins and renews paging',
    () async {
      final repository = _ReorderRepository([
        _clip('a'),
        _clip('b'),
        _clip('c'),
        _clip('d'),
        _clip('loose', pinned: false),
      ]);
      final controller = HistoryController(repository, pageSize: 2);
      addTearDown(controller.dispose);
      addTearDown(repository.events.close);
      await controller.initialize();
      await controller.select('a');
      final states = <HistoryLoadState>[];
      controller.addListener(() => states.add(controller.state));

      expect(await controller.movePinned('a', 'b', before: false), isTrue);
      expect(repository.orders, [
        ['b', 'a'],
      ]);
      expect(_ids(repository.items), ['b', 'a', 'c', 'd', 'loose']);
      expect(controller.selectedId, 'a');
      expect(controller.selectedClip?.id, 'a');
      expect(states, everyElement(HistoryLoadState.ready));
      expect(repository.requests.last.$2, isNull);
      await controller.loadMore();
      expect(_ids(controller.items), ['b', 'a', 'c', 'd']);
      expect(repository.requests.last.$2, '2');
    },
  );

  test(
    'rolls back a failed save and blocks concurrent pin mutations',
    () async {
      final repository =
          _ReorderRepository([_clip('a'), _clip('b'), _clip('c')])
            ..saveGate = Completer<void>()
            ..saveError = StateError('save failed');
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      addTearDown(repository.events.close);
      await controller.initialize();
      final saving = controller.movePinned('c', 'a', before: true);
      expect(_ids(controller.items), ['c', 'a', 'b']);
      expect(controller.isReorderingPinned, isTrue);
      expect(controller.canReorderPinned, isFalse);
      expect(await controller.movePinned('b', 'a', before: true), isFalse);
      expect(await controller.togglePin(controller.items.first), isFalse);
      expect(await controller.deleteClip('a'), isFalse);
      expect(await controller.deleteAll(), isFalse);

      repository.saveGate!.complete();
      expect(await saving, isFalse);
      expect(_ids(controller.items), ['a', 'b', 'c']);
      expect(controller.isReorderingPinned, isFalse);
      expect(controller.errorMessage, contains('could not be saved'));
      expect(repository.orders, hasLength(1));
    },
  );

  test('defers watcher refresh until a cancelled drag ends', () async {
    final repository = _ReorderRepository([_clip('a'), _clip('b')]);
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    addTearDown(repository.events.close);
    await controller.initialize();
    controller.beginPinnedDrag('a');
    repository.items.add(_clip('c'));
    repository.events.add(HistoryRuntimeEvent.itemsChanged);
    await Future<void>.delayed(Duration.zero);
    expect(repository.requests, hasLength(1));
    expect(_ids(controller.items), ['a', 'b']);
    controller.endPinnedDrag();
    await Future<void>.delayed(Duration.zero);
    expect(_ids(controller.items), ['a', 'b', 'c']);
    expect(repository.orders, isEmpty);
    expect(controller.draggedPinnedId, isNull);
  });

  test(
    'a refresh already in flight cannot replace pins during dragging',
    () async {
      final repository = _ReorderRepository([_clip('a'), _clip('b')]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      addTearDown(repository.events.close);
      await controller.initialize();
      repository.queryGate = Completer<void>();
      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);
      controller.beginPinnedDrag('a');
      repository.items.add(_clip('c'));
      repository.queryGate!.complete();
      await Future<void>.delayed(Duration.zero);
      expect(_ids(controller.items), ['a', 'b']);
      expect(controller.draggedPinnedId, 'a');
      controller.endPinnedDrag();
      await Future<void>.delayed(Duration.zero);
      expect(_ids(controller.items), ['a', 'b', 'c']);
    },
  );

  test('a query change during saving wins over the reorder refresh', () async {
    final repository = _ReorderRepository([_clip('a'), _clip('b')])
      ..saveGate = Completer<void>();
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    addTearDown(repository.events.close);
    await controller.initialize();
    final saving = controller.movePinned('a', 'b', before: false);
    await controller.updateQuery(const HistoryQuery(search: 'new query'));
    repository.saveGate!.complete();
    await saving;
    await Future<void>.delayed(Duration.zero);
    expect(repository.requests.last.$1.search, 'new query');
    expect(controller.state, HistoryLoadState.ready);
    expect(controller.isReorderingPinned, isFalse);
  });

  test('keeps the committed order when only the refresh fails', () async {
    final repository = _ReorderRepository([_clip('a'), _clip('b')]);
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    addTearDown(repository.events.close);
    await controller.initialize();
    repository.queryError = StateError('refresh failed');
    expect(await controller.movePinned('a', 'b', before: false), isTrue);
    expect(_ids(controller.items), ['b', 'a']);
    expect(_ids(repository.items), ['b', 'a']);
    expect(controller.errorMessage, contains('order was saved'));
    expect(controller.state, HistoryLoadState.ready);
  });

  for (final query in [
    const HistoryQuery(search: 'clip'),
    const HistoryQuery(kind: HistoryClipKind.text),
    const HistoryQuery(pinnedOnly: true),
    const HistoryQuery(origin: 'device'),
    const HistoryQuery(sourceApp: 'editor'),
  ]) {
    test('rejects reordering in a filtered query: $query', () async {
      final repository = _ReorderRepository([_clip('a'), _clip('b')]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      addTearDown(repository.events.close);
      await controller.updateQuery(query);
      expect(controller.hasUnfilteredQuery, isFalse);
      expect(await controller.movePinned('a', 'b', before: false), isFalse);
      expect(repository.orders, isEmpty);
    });
  }

  test(
    'ignores invalid or unchanged drops and supports keyboard steps',
    () async {
      final repository = _ReorderRepository([
        _clip('a'),
        _clip('b'),
        _clip('loose', pinned: false),
      ]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      addTearDown(repository.events.close);
      await controller.initialize();
      expect(await controller.movePinned('a', 'a', before: true), isFalse);
      expect(await controller.movePinned('a', 'b', before: true), isFalse);
      expect(await controller.movePinned('loose', 'b', before: true), isFalse);
      expect(await controller.shiftPinned('a', up: true), isFalse);
      expect(repository.orders, isEmpty);
      expect(await controller.shiftPinned('a', up: false), isTrue);
      expect(_ids(controller.items), ['b', 'a', 'loose']);
    },
  );

  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.android,
  ]) {
    testWidgets(
      'drags only by the handle and highlights insertion on $platform',
      (tester) async {
        final repository = _ReorderRepository([
          _clip('a'),
          _clip('b'),
          _clip('c'),
          _clip('loose', pinned: false),
        ]);
        final controller = await _pumpScreen(tester, repository, platform);
        final mouse = platform == TargetPlatform.android
            ? null
            : await _hover(tester, 'a');
        final handle = _handle('a');
        expect(handle, findsOneWidget);
        if (platform == TargetPlatform.android) {
          expect(
            tester.getSize(handle),
            const Size.square(AppControlSize.touch),
          );
          expect(
            find.byKey(const ValueKey<String>('history-row-delete-a')),
            findsNothing,
          );
        } else {
          final pin = tester.getCenter(
            find.byKey(const ValueKey<String>('history-row-pin-a')),
          );
          final delete = tester.getCenter(
            find.byKey(const ValueKey<String>('history-row-delete-a')),
          );
          expect(pin.dx, lessThan(delete.dx));
          expect(delete.dx, lessThan(tester.getCenter(handle).dx));
        }

        final kind = platform == TargetPlatform.android
            ? PointerDeviceKind.touch
            : PointerDeviceKind.mouse;
        final gesture = mouse ?? await tester.createGesture(kind: kind);
        await gesture.down(tester.getCenter(handle));
        await gesture.moveBy(const Offset(0, 24));
        await tester.pump();
        final target = tester.getRect(_rowContent('c'));
        await gesture.moveTo(Offset(target.center.dx, target.bottom - 4));
        await tester.pump(const Duration(milliseconds: 250));
        expect(controller.draggedPinnedId, 'a');
        final placeholder = find.byKey(
          const ValueKey<String>('history-pin-drop-placeholder'),
        );
        expect(placeholder, findsOneWidget);
        final decoration =
            tester.widget<DecoratedBox>(placeholder).decoration
                as BoxDecoration;
        expect(decoration.border, isNotNull);
        expect(decoration.color, isNotNull);
        expect(repository.orders, isEmpty);
        expect(controller.selectedId, isNull);
        await gesture.up();
        if (mouse != null) await mouse.moveTo(Offset.zero);
        await _pumpActions(tester);
        expect(repository.orders, [
          ['b', 'c', 'a'],
        ]);
        expect(_ids(controller.items), ['b', 'c', 'a', 'loose']);
        expect(controller.draggedPinnedId, isNull);
        expect(placeholder, findsNothing);
        expect(tester.takeException(), isNull);
      },
    );
  }

  testWidgets('cancels outside Pinned without saving or selecting a clip', (
    tester,
  ) async {
    final repository = _ReorderRepository([
      _clip('a'),
      _clip('b'),
      _clip('loose', pinned: false),
    ]);
    final controller = await _pumpScreen(
      tester,
      repository,
      TargetPlatform.macOS,
    );
    final mouse = await _hover(tester, 'a');
    await mouse.down(tester.getCenter(_handle('a')));
    await mouse.moveBy(const Offset(0, 24));
    await tester.pump();
    await mouse.moveTo(tester.getCenter(_rowContent('loose')));
    await tester.pump();
    expect(
      find.byKey(const ValueKey<String>('history-pin-drop-placeholder')),
      findsNothing,
    );
    await mouse.up();
    await mouse.moveTo(Offset.zero);
    await _pumpActions(tester);
    expect(repository.orders, isEmpty);
    expect(_ids(controller.items), ['a', 'b', 'loose']);
    expect(controller.selectedId, isNull);
    expect(controller.draggedPinnedId, isNull);
    expect(tester.takeException(), isNull);
  });

  testWidgets('scrolls and loads more pins while dragging near the edge', (
    tester,
  ) async {
    final repository = _ReorderRepository(
      List.generate(36, (index) => _clip('pin-$index')),
    );
    final controller = await _pumpScreen(
      tester,
      repository,
      TargetPlatform.android,
      pageSize: 10,
    );
    final scroll = tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byType(ScrollableSortableLayer),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    final gesture = await tester.startGesture(
      tester.getCenter(_handle('pin-0')),
    );
    await gesture.moveBy(const Offset(0, 24));
    await tester.pump();
    final viewport = tester.getRect(find.byType(ScrollableSortableLayer));
    await gesture.moveTo(
      Offset(viewport.center.dx, viewport.bottom - AppSpacing.sm),
    );
    for (var frame = 0; frame < 18; frame++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    expect(scroll.position.pixels, greaterThan(0));
    expect(controller.items.length, greaterThan(10));
    await gesture.cancel();
    await tester.pumpAndSettle();
    expect(repository.orders, isEmpty);
    expect(controller.draggedPinnedId, isNull);
    expect(tester.takeException(), isNull);
  });

  testWidgets('hides handles during search and keeps row content selectable', (
    tester,
  ) async {
    final repository = _ReorderRepository([_clip('a'), _clip('b')]);
    final controller = await _pumpScreen(
      tester,
      repository,
      TargetPlatform.android,
    );
    await controller.updateQuery(const HistoryQuery(search: 'clip'));
    await tester.pumpAndSettle();
    expect(_handle('a'), findsNothing);
    await tester.tap(_rowContent('a'));
    await tester.pumpAndSettle();
    expect(controller.selectedId, 'a');
  });

  testWidgets('scrolling a clip body never starts a reorder', (tester) async {
    final repository = _ReorderRepository(
      List.generate(30, (index) => _clip('pin-$index')),
    );
    final controller = await _pumpScreen(
      tester,
      repository,
      TargetPlatform.android,
    );
    await tester.drag(_rowContent('pin-0'), const Offset(0, -120));
    await tester.pumpAndSettle();
    final scroll = tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byType(ScrollableSortableLayer),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    expect(scroll.position.pixels, greaterThan(0));
    expect(repository.orders, isEmpty);
    expect(controller.draggedPinnedId, isNull);
    expect(controller.selectedId, isNull);
  });

  testWidgets('keyboard focus reveals the handle and Alt + Down moves a pin', (
    tester,
  ) async {
    final repository = _ReorderRepository([_clip('a'), _clip('b')]);
    await _pumpScreen(tester, repository, TargetPlatform.windows);
    expect(_handle('a'), findsNothing);
    Focus.of(tester.element(_rowContent('a'))).requestFocus();
    await _pumpActions(tester);
    expect(_handle('a'), findsOneWidget);
    final icon = find.descendant(of: _handle('a'), matching: find.byType(Icon));
    Focus.of(tester.element(icon)).requestFocus();
    await tester.pump();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    await _pumpActions(tester);
    expect(repository.orders, [
      ['b', 'a'],
    ]);
    expect(tester.takeException(), isNull);
  });
}

Finder _handle(String id) =>
    find.byKey(ValueKey<String>('history-row-reorder-$id'));
Finder _rowContent(String id) =>
    find.byKey(ValueKey<String>('history-clip-$id'));
List<String> _ids(List<HistoryClip> clips) =>
    clips.map((clip) => clip.id).toList();

HistoryClip _clip(String id, {bool pinned = true}) => HistoryClip(
  id: id,
  contentType: 'text',
  preview: 'Clip $id',
  createdAt: DateTime.utc(2026),
  pinned: pinned,
);

Future<HistoryController> _pumpScreen(
  WidgetTester tester,
  _ReorderRepository repository,
  TargetPlatform platform, {
  int pageSize = 50,
}) async {
  await tester.binding.setSurfaceSize(
    platform == TargetPlatform.android
        ? const Size(390, 640)
        : const Size(1400, 800),
  );
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final controller = HistoryController(repository, pageSize: pageSize);
  addTearDown(controller.dispose);
  addTearDown(repository.events.close);
  await tester.pumpWidget(
    ShadcnApp(
      theme: AppTheme.light.copyWith(platform: () => platform),
      builder: AppTheme.builder,
      home: Scaffold(child: HistoryScreen(controller: controller)),
    ),
  );
  await tester.pumpAndSettle();
  return controller;
}

Future<TestGesture> _hover(WidgetTester tester, String id) async {
  final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
  await mouse.addPointer(location: Offset.zero);
  addTearDown(mouse.removePointer);
  await mouse.moveTo(tester.getCenter(_rowContent(id)));
  await _pumpActions(tester);
  return mouse;
}

Future<void> _pumpActions(WidgetTester tester) async {
  // Hover/focus keeps shadcn tooltip anchor tracking active. Flush the drop and
  // button transitions without waiting for that persistent ticker to stop.
  await tester.pump();
  await tester.pump(const Duration(seconds: 1));
  await tester.pump(const Duration(milliseconds: 200));
}

class _ReorderRepository implements HistoryRepository {
  _ReorderRepository(this.items);
  final List<HistoryClip> items;
  final events = StreamController<HistoryRuntimeEvent>.broadcast();
  final List<List<String>> orders = [];
  final List<(HistoryQuery, String?)> requests = [];
  Completer<void>? saveGate;
  Completer<void>? queryGate;
  Object? saveError;
  Object? queryError;

  @override
  Stream<HistoryRuntimeEvent> watch() => events.stream;
  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();
  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    requests.add((query, cursor));
    await queryGate?.future;
    if (queryError != null) throw queryError!;
    final start = int.parse(cursor ?? '0');
    final end = (start + limit).clamp(0, items.length);
    return HistoryClipPage(
      items: items.sublist(start, end),
      nextCursor: end < items.length ? '$end' : null,
    );
  }

  @override
  Future<void> reorderPinned(List<String> ids) async {
    orders.add(List.of(ids));
    await saveGate?.future;
    if (saveError != null) throw saveError!;
    final pins = {
      for (final clip in items.where((clip) => clip.pinned)) clip.id: clip,
    };
    final tail = items.where((clip) => !ids.contains(clip.id)).toList();
    items
      ..clear()
      ..addAll(ids.map((id) => pins[id]).whereType<HistoryClip>())
      ..addAll(tail);
    events.add(HistoryRuntimeEvent.itemsChanged);
  }

  @override
  Future<HistoryClip> get(String id) async =>
      items.firstWhere((clip) => clip.id == id);
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
  Future<void> setPinned(String id, bool pinned) async {}
  @override
  Future<void> delete(String id) async {}
  @override
  Future<void> deleteAll() async {}
}
