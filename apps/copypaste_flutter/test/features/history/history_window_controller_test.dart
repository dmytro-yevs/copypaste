import 'dart:async';

import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
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

void main() {
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
      await controller.reportPageExtent(0, 100);
      controller.beforeWindowChange = () async => {'1'};

      repo.clips.insertAll(0, [for (var i = 10000; i < 10008; i++) _clip(i)]);
      repo.events.add(HistoryRuntimeEvent.itemsChanged);
      for (var i = 0; i < 20; i++) {
        await Future<void>.delayed(Duration.zero);
      }

      expect(controller.state, HistoryLoadState.ready);
      expect(controller.items.any((clip) => clip.id == '1'), isTrue);
      expect(
        controller.windowPages.every(
          (page) => page.items != null || page.hasExactExtent,
        ),
        isTrue,
        reason:
            'A rebuilt page must remain renderable until it can be measured.',
      );
      for (var index = 0; index < controller.windowPages.length; index++) {
        await controller.reportPageExtent(index, 100);
      }
      expect(
        controller.windowPages.where((page) => page.items != null).length,
        lessThanOrEqualTo(3),
        reason: 'Measured rebuilt pages still obey the resident-page limit.',
      );
      expect(controller.items.any((clip) => clip.id == '1'), isTrue);
    },
  );

  test(
    'bounds rows across 10000 visits and refetches backwards by saved cursor',
    () async {
      final repo = _Repository();
      final controller = HistoryController(repo, includeFacets: false);
      addTearDown(controller.dispose);
      await controller.initialize();
      controller.reportPageExtent(0, 100);
      await controller.select('0');
      for (var page = 1; page < 200; page++) {
        await controller.loadMore();
        controller.reportPageExtent(page, 100);
        expect(controller.items.length, lessThanOrEqualTo(150));
      }
      expect(controller.visitedIds, hasLength(10000));
      expect(controller.selectedId, '0');
      expect(controller.selectedClip?.id, '0');
      expect(controller.canLoadPrevious, isTrue);
      await controller.loadPrevious();
      expect(repo.requests.last, '9800');
      expect(controller.items.first.id, '9800');
    },
  );

  test('deletes selected IDs after their pages are evicted', () async {
    final repo = _Repository();
    final controller = HistoryController(repo, includeFacets: false);
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    await controller.initialize();
    controller.reportPageExtent(0, 100);
    controller.beginBulkSelection('0');
    for (var page = 1; page < 5; page++) {
      await controller.loadMore();
      controller.reportPageExtent(page, 100);
    }
    controller.toggleBulkSelection('200');
    expect(controller.items.any((item) => item.id == '0'), isFalse);
    expect(controller.bulkSelectedIds, {'0', '200'});
    expect(await controller.deleteBulkSelection(), isTrue);
    expect(repo.deleted.toSet(), {'0', '200'});
  });
  test('rebuilds changed cursor boundaries around resident rows while preserving selection', () async {
    final repo = _MutableRepository();
    final controller = HistoryController(repo, includeFacets: false);
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    await controller.initialize();
    controller.reportPageExtent(0, 100);
    await controller.select('0');
    for (var i = 0; i < 4; i++) {
      await controller.loadMore();
      controller.reportPageExtent(i + 1, 100);
    }
    final anchor = controller.items.first.id;
    repo.clips.insert(0, _clip(10000));
    repo.events.add(HistoryRuntimeEvent.itemsChanged);
    for (var i = 0; i < 20; i++) {
      await Future<void>.delayed(Duration.zero);
    }
    expect(controller.items.any((clip) => clip.id == anchor), isTrue);
    expect(controller.selectedId, '0');
    expect(controller.selectedClip?.id, '0');
    expect(controller.items.length, lessThanOrEqualTo(150));
  });
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
    controller.reportPageExtent(0, 100);
    await controller.select('0');
    controller.toggleSection('yesterday');
    for (var i = 0; i < 3; i++) {
      await controller.loadMore();
      controller.reportPageExtent(i + 1, 100);
    }
    controller.selectBulkClip('150', orderedIds: ['150'], range: true);
    expect(controller.bulkSelectedIds, contains('0'));
    expect(controller.bulkSelectedIds, contains('150'));
    expect(controller.bulkSelectedIds, isNot(contains('75')));
    expect(controller.bulkSelectedIds, contains('125'));
  });
}
