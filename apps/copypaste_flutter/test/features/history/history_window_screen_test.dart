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

void main() {
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
    await controller.loadMore();
    await controller.loadMore();
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
    final load = controller.loadMore();
    await tester.pumpAndSettle();
    await load;
    await tester.pumpAndSettle();
    final row = find.byKey(ValueKey('history-row-$visible'));
    expect(row, findsOneWidget);
    expect(tester.getTopLeft(row).dy, closeTo(y!, 1));
    expect(controller.canLoadPrevious, isTrue);
    final previous = controller.loadPrevious();
    await tester.pumpAndSettle();
    await previous;
    await tester.pumpAndSettle();
    expect(row, findsOneWidget);
    expect(tester.getTopLeft(row).dy, closeTo(y, 1));
    final next = find.byKey(ValueKey('history-row-${int.parse(visible!) + 1}'));
    expect(next, findsOneWidget);
    final nextY = tester.getTopLeft(next).dy;
    final deletion = controller.deleteClip(visible);
    await tester.pumpAndSettle();
    expect(await deletion, isTrue);
    await tester.pumpAndSettle();
    expect(next, findsOneWidget);
    expect(tester.getTopLeft(next).dy, closeTo(nextY, 1));
  });
}
