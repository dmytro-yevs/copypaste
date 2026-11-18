import 'dart:async';

import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_window_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:flutter_test/flutter_test.dart';

HistoryClip _clip(int index) => HistoryClip(
  id: '$index',
  contentType: 'text/plain',
  preview: 'clip $index',
  createdAt: DateTime.utc(2026),
  pinned: false,
);

class _Repository extends Fake implements HistoryRepository {
  final events = StreamController<HistoryRuntimeEvent>.broadcast();
  final List<String> deleted = [];
  final requests = <String?>[];
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
    requests.add(cursor);
    final start = int.tryParse(cursor ?? '') ?? 0;
    final end = (start + limit).clamp(0, 10000);
    return HistoryClipPage(
      items: [for (var i = start; i < end; i++) _clip(i)],
      nextCursor: end == 10000 ? null : '$end',
    );
  }

  @override
  Future<HistoryClip> get(String id) async => _clip(int.parse(id));
  @override
  Future<void> delete(String id) async {
    deleted.add(id);
  }
}

class _MutableRepository extends Fake implements HistoryRepository {
  final events = StreamController<HistoryRuntimeEvent>.broadcast();
  final clips = <HistoryClip>[for (var i = 0; i < 250; i++) _clip(i)];
  final requests = <String?>[];
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
    requests.add(cursor);
    final start = int.tryParse(cursor ?? '') ?? 0;
    final end = (start + limit).clamp(0, clips.length);
    return HistoryClipPage(
      items: clips.sublist(start, end),
      nextCursor: end == clips.length ? null : '$end',
    );
  }

  @override
  Future<HistoryClip> get(String id) async =>
      clips.firstWhere((clip) => clip.id == id);
}

Future<HistoryController> _scrolledController(_MutableRepository repo) async {
  repo.clips.addAll([for (var i = 250; i < 10000; i++) _clip(i)]);
  final controller = HistoryController(repo, includeFacets: false);
  addTearDown(controller.dispose);
  addTearDown(repo.events.close);
  await controller.initialize();
  await controller.window.reportPageExtent(0, 100);
  for (var page = 1; page < 4; page++) {
    await controller.window.loadMore();
    await controller.window.reportPageExtent(page, 100);
  }
  return controller;
}

Future<void> _settle() async {
  for (var i = 0; i < 40; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

void main() {
  test(
    'rebuild stops just past a vanished anchor and keeps its neighbours',
    () async {
      final repo = _MutableRepository();
      final controller = await _scrolledController(repo);
      controller.window.beforeWindowChange = () async => {'120'};
      repo.clips
        ..removeWhere((clip) => clip.id == '120')
        ..insert(0, _clip(20000));
      repo.requests.clear();
      repo.events.add(HistoryRuntimeEvent.itemsChanged);
      await _settle();

      expect(controller.window.state, HistoryLoadState.ready);
      expect(
        repo.requests.length,
        lessThanOrEqualTo(5),
        reason: 'A missing anchor must not scan the remaining 196 pages.',
      );
      final pages = controller.window.windowPages;
      final fallback = pages.indexWhere((page) => page.ids.contains('121'));
      expect(fallback, greaterThan(0));
      expect(pages[fallback - 1].resident, isTrue);
      expect(pages[fallback].resident, isTrue);
      expect(pages[fallback + 1].resident, isTrue);
    },
  );

  test('rebuild follows an anchor moved hundreds of rows down', () async {
    final repo = _MutableRepository();
    final controller = await _scrolledController(repo);
    controller.window.beforeWindowChange = () async => {'100'};
    repo.clips.insertAll(0, [for (var i = 20000; i < 20300; i++) _clip(i)]);
    repo.events.add(HistoryRuntimeEvent.itemsChanged);
    await _settle();

    final pages = controller.window.windowPages;
    final anchor = pages.indexWhere((page) => page.ids.contains('100'));
    expect(anchor, 8);
    expect(pages[anchor].resident, isTrue);
    expect(controller.window.items.any((clip) => clip.id == '100'), isTrue);
  });

  test('a recopied anchor below the first keeps the user\'s place', () async {
    final repo = _MutableRepository();
    repo.clips.removeRange(120, repo.clips.length);
    final controller = HistoryController(repo, includeFacets: false);
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    await controller.initialize();
    await controller.window.reportPageExtent(0, 100);
    for (var page = 1; page < 3; page++) {
      await controller.window.loadMore();
      await controller.window.reportPageExtent(page, 100);
    }
    controller.window.beforeWindowChange = () async => {'100', '105'};
    final recopied = repo.clips.firstWhere((clip) => clip.id == '105');
    repo.clips
      ..remove(recopied)
      ..insert(
        0,
        HistoryClip(
          id: recopied.id,
          contentType: recopied.contentType,
          preview: recopied.preview,
          createdAt: DateTime.utc(2027),
          pinned: false,
        ),
      );
    repo.events.add(HistoryRuntimeEvent.itemsChanged);
    await _settle();

    final pages = controller.window.windowPages;
    final anchor = pages.indexWhere((page) => page.ids.contains('100'));
    expect(anchor, 2);
    expect(pages[anchor].resident, isTrue);
    expect(controller.window.items.any((clip) => clip.id == '100'), isTrue);
  });

  test(
    'keeps rebuilt pages renderable before their geometry is measured',
    () async {
      final repo = _MutableRepository();
      final controller = HistoryController(
        repo,
        pageSize: 2,
        includeFacets: false,
      );
      addTearDown(controller.dispose);
      addTearDown(repo.events.close);
      await controller.initialize();
      await controller.window.reportPageExtent(0, 100);
      controller.window.beforeWindowChange = () async => {'1'};

      repo.clips.insertAll(0, [for (var i = 10000; i < 10008; i++) _clip(i)]);
      repo.events.add(HistoryRuntimeEvent.itemsChanged);
      for (var i = 0; i < 20; i++) {
        await Future<void>.delayed(Duration.zero);
      }

      expect(controller.window.state, HistoryLoadState.ready);
      expect(controller.window.items.any((clip) => clip.id == '1'), isTrue);
      expect(
        controller.window.windowPages.every(
          (page) => page.items != null || page.hasExactExtent,
        ),
        isTrue,
        reason:
            'A rebuilt page must remain renderable until it can be measured.',
      );
      for (
        var index = 0;
        index < controller.window.windowPages.length;
        index++
      ) {
        await controller.window.reportPageExtent(index, 100);
      }
      expect(
        controller.window.windowPages
            .where((page) => page.items != null)
            .length,
        lessThanOrEqualTo(3),
        reason: 'Measured rebuilt pages still obey the resident-page limit.',
      );
      expect(controller.window.items.any((clip) => clip.id == '1'), isTrue);
    },
  );

  test(
    'bounds rows across 10000 visits and refetches backwards by saved cursor',
    () async {
      final repo = _Repository();
      final controller = HistoryController(repo, includeFacets: false);
      addTearDown(controller.dispose);
      await controller.initialize();
      controller.window.reportPageExtent(0, 100);
      await controller.selection.select('0');
      for (var page = 1; page < 200; page++) {
        await controller.window.loadMore();
        controller.window.reportPageExtent(page, 100);
        expect(controller.window.items.length, lessThanOrEqualTo(150));
      }
      expect(controller.window.visitedIds, hasLength(10000));
      expect(controller.selection.selectedId, '0');
      expect(controller.selection.selectedClip?.id, '0');
      expect(controller.window.canLoadPrevious, isTrue);
      await controller.window.loadPrevious();
      expect(repo.requests.last, '9800');
      expect(controller.window.items.first.id, '9800');
    },
  );

  test('deletes selected IDs after their pages are evicted', () async {
    final repo = _Repository();
    final controller = HistoryController(repo, includeFacets: false);
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    await controller.initialize();
    controller.window.reportPageExtent(0, 100);
    controller.bulk.beginBulkSelection('0');
    for (var page = 1; page < 5; page++) {
      await controller.window.loadMore();
      controller.window.reportPageExtent(page, 100);
    }
    controller.bulk.toggleBulkSelection('200');
    expect(controller.window.items.any((item) => item.id == '0'), isFalse);
    expect(controller.bulk.bulkSelectedIds, {'0', '200'});
    expect(await controller.bulk.deleteBulkSelection(), isTrue);
    expect(repo.deleted.toSet(), {'0', '200'});
  });
  test(
    'rebuilds changed cursor boundaries around resident rows while preserving selection',
    () async {
      final repo = _MutableRepository();
      final controller = HistoryController(repo, includeFacets: false);
      addTearDown(controller.dispose);
      addTearDown(repo.events.close);
      await controller.initialize();
      controller.window.reportPageExtent(0, 100);
      await controller.selection.select('0');
      for (var i = 0; i < 4; i++) {
        await controller.window.loadMore();
        controller.window.reportPageExtent(i + 1, 100);
      }
      final anchor = controller.window.items.first.id;
      repo.clips.insert(0, _clip(10000));
      repo.events.add(HistoryRuntimeEvent.itemsChanged);
      for (var i = 0; i < 20; i++) {
        await Future<void>.delayed(Duration.zero);
      }
      expect(controller.window.items.any((clip) => clip.id == anchor), isTrue);
      expect(controller.selection.selectedId, '0');
      expect(controller.selection.selectedClip?.id, '0');
      expect(controller.window.items.length, lessThanOrEqualTo(150));
    },
  );
  test('range across evicted pages excludes collapsed date sections', () async {
    final repo = _MutableRepository();
    final now = DateTime.now();
    repo.clips
      ..clear()
      ..addAll([
        for (var i = 0; i < 200; i++)
          HistoryClip(
            id: '$i',
            contentType: 'text/plain',
            preview: 'clip $i',
            createdAt: DateTime(
              now.year,
              now.month,
              now.day -
                  (i < 50
                      ? 0
                      : i < 100
                      ? 1
                      : 2),
              12,
            ),
            pinned: false,
          ),
      ]);
    final controller = HistoryController(repo, includeFacets: false);
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    await controller.initialize();
    controller.window.reportPageExtent(0, 100);
    await controller.selection.select('0');
    controller.window.toggleSection('yesterday');
    for (var i = 0; i < 3; i++) {
      await controller.window.loadMore();
      controller.window.reportPageExtent(i + 1, 100);
    }
    controller.bulk.selectBulkClip('150', orderedIds: ['150'], range: true);
    expect(controller.bulk.bulkSelectedIds, contains('0'));
    expect(controller.bulk.bulkSelectedIds, contains('150'));
    expect(controller.bulk.bulkSelectedIds, isNot(contains('75')));
    expect(controller.bulk.bulkSelectedIds, contains('125'));
  });
}
