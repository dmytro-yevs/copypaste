import 'dart:async';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

class _Repository extends Fake implements HistoryRepository {
  final deleted = <String>{};
  final clips = <HistoryClip>[
    for (var i = 0; i < 100; i++)
      HistoryClip(
        id: '$i',
        contentType: 'text/plain',
        preview: 'clip $i',
        createdAt: DateTime.utc(2026).subtract(Duration(seconds: i)),
        pinned: false,
      ),
  ];
  final events = StreamController<HistoryRuntimeEvent>.broadcast();
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
    final start = cursor == null ? 0 : int.parse(cursor) + 1;
    final matching = clips
        .where(
          (clip) => !deleted.contains(clip.id) && int.parse(clip.id) >= start,
        )
        .take(limit)
        .toList(growable: false);
    return HistoryClipPage(
      items: matching,
      nextCursor: matching.length == limit ? matching.last.id : null,
    );
  }

  @override
  Future<void> delete(String id) async {
    deleted.add(id);
  }
}

class _PrependingRepository extends _Repository {
  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    final start = int.tryParse(cursor ?? '') ?? 0;
    final end = (start + limit).clamp(0, clips.length);
    return HistoryClipPage(
      items: clips.sublist(start, end),
      nextCursor: end == clips.length ? null : '$end',
    );
  }
}

class _Retained extends StatefulWidget {
  const _Retained({required this.child});

  final Widget child;

  @override
  State<_Retained> createState() => _RetainedState();
}

class _RetainedState extends State<_Retained>
    with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return widget.child;
  }
}

void main() {
  testWidgets('keeps its place when a clip arrives on another destination', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1000, 600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repo = _PrependingRepository();
    final controller = HistoryController(repo);
    final destinations = PageController();
    final scroll = ScrollController();
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    addTearDown(destinations.dispose);
    addTearDown(scroll.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: PageView(
            controller: destinations,
            allowImplicitScrolling: true,
            children: [
              _Retained(
                child: PrimaryScrollController(
                  controller: scroll,
                  automaticallyInheritForPlatforms: const {},
                  child: HistoryScreen(controller: controller),
                ),
              ),
              const SizedBox.shrink(),
              const SizedBox.shrink(),
            ],
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    scroll.jumpTo(120);
    await tester.pumpAndSettle();
    final row = find.byKey(const ValueKey('history-row-3'));
    final y = tester.getTopLeft(row).dy;

    destinations.jumpToPage(2);
    await tester.pumpAndSettle();
    repo.clips.insert(
      0,
      HistoryClip(
        id: 'new',
        contentType: 'text/plain',
        preview: 'new',
        createdAt: DateTime.utc(2026, 1, 1, 0, 0, 1),
        pinned: false,
      ),
    );
    repo.events.add(HistoryRuntimeEvent.itemsChanged);
    await tester.pumpAndSettle();
    destinations.jumpToPage(0);
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(row, findsOneWidget);
    expect(tester.getTopLeft(row).dy, closeTo(y, 1));
  });

  testWidgets('renders a rebuilt prefix after history was offstage', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1000, 300));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repo = _PrependingRepository();
    final controller = HistoryController(repo, pageSize: 2);
    final hidden = ValueNotifier(false);
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    addTearDown(hidden.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: ValueListenableBuilder<bool>(
            valueListenable: hidden,
            child: HistoryScreen(controller: controller),
            builder: (context, value, child) =>
                Offstage(offstage: value, child: child),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    hidden.value = true;
    await tester.pump();
    controller.window.beforeWindowChange = () async => {'1'};
    repo.clips.insertAll(0, [
      for (final id in ['new-1', 'new-2'])
        HistoryClip(
          id: id,
          contentType: 'text/plain',
          preview: id,
          createdAt: DateTime.utc(2026),
          pinned: false,
        ),
    ]);
    repo.events.add(HistoryRuntimeEvent.itemsChanged);
    await tester.pumpAndSettle();
    hidden.value = false;
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.byKey(const ValueKey('history-row-new-1')), findsOneWidget);
    expect(find.text('Loading'), findsNothing);
    expect(find.text('No clips found'), findsNothing);
  });

  testWidgets('retains the visible row position when older pages are evicted', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1000, 600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repo = _Repository();
    final controller = HistoryController(repo, pageSize: 10);
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();
    await controller.window.loadMore();
    await controller.window.loadMore();
    await tester.pumpAndSettle();
    final list = tester.widget<CustomScrollView>(
      find.byType(CustomScrollView).first,
    );
    final scroll = list.controller!;
    scroll.jumpTo(scroll.position.maxScrollExtent / 2);
    await tester.pumpAndSettle();
    final viewport = tester.getRect(find.byType(CustomScrollView).first);
    String? visible;
    double? y;
    for (var id = 0; id < 30; id++) {
      final row = find.byKey(ValueKey('history-row-$id'));
      if (row.evaluate().isEmpty) continue;
      final rect = tester.getRect(row);
      if (rect.overlaps(viewport)) {
        visible = '$id';
        y = rect.top;
        break;
      }
    }
    expect(visible, isNotNull);
    final load = controller.window.loadMore();
    await tester.pumpAndSettle();
    await load;
    await tester.pumpAndSettle();
    final row = find.byKey(ValueKey('history-row-$visible'));
    expect(row, findsOneWidget);
    expect(tester.getTopLeft(row).dy, closeTo(y!, 1));
    expect(controller.window.canLoadPrevious, isTrue);
    final previous = controller.window.loadPrevious();
    await tester.pumpAndSettle();
    await previous;
    await tester.pumpAndSettle();
    expect(row, findsOneWidget);
    expect(tester.getTopLeft(row).dy, closeTo(y, 1));
    final next = find.byKey(ValueKey('history-row-${int.parse(visible!) + 1}'));
    expect(next, findsOneWidget);
    final nextY = tester.getTopLeft(next).dy;
    final deletion = controller.mutations.deleteClip(visible);
    await tester.pumpAndSettle();
    expect(await deletion, isTrue);
    await tester.pumpAndSettle();
    expect(next, findsOneWidget);
    expect(tester.getTopLeft(next).dy, closeTo(nextY, 1));
  });
}
