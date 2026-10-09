import 'dart:async';

import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  late _BulkRepository repository;
  late HistoryController controller;

  setUp(() async {
    repository = _BulkRepository();
    controller = HistoryController(repository, pageSize: 2);
    await controller.initialize();
  });

  tearDown(() async {
    controller.dispose();
    await repository.events.close();
  });

  test(
    'Shift range uses visual order and shrinks from a stable anchor',
    () async {
      await controller.loadMore();
      await controller.select('one');
      const order = ['one', 'three', 'two'];
      controller.selectBulkClip('two', orderedIds: order, range: true);
      expect(controller.bulkSelectedIds, {'one', 'three', 'two'});
      expect(controller.selectedId, isNull);
      controller.selectBulkClip('three', orderedIds: order, range: true);
      expect(controller.bulkSelectedIds, {'one', 'three'});
    },
  );

  test(
    'Ctrl adds to the inspector selection and Ctrl Shift preserves other clips',
    () async {
      await controller.loadMore();
      await controller.select('one');
      const order = ['one', 'two', 'three'];
      controller.selectBulkClip('three', orderedIds: order, additive: true);
      expect(controller.bulkSelectedIds, {'one', 'three'});
      controller.selectBulkClip(
        'two',
        orderedIds: order,
        additive: true,
        range: true,
      );
      expect(controller.bulkSelectedIds, {'one', 'two', 'three'});
      controller.selectBulkClip('one', orderedIds: order, additive: true);
      expect(controller.bulkSelectedIds, {'two', 'three'});
    },
  );

  test(
    'held range reverses without toggling clips or losing its original selection',
    () async {
      controller.beginBulkSelection('two');
      expect(controller.beginBulkDragSelection('one'), isTrue);
      expect(controller.canSuspend, isFalse);
      await controller.loadMore();
      const order = ['one', 'two', 'three'];
      controller.updateBulkDragSelection('three', orderedIds: order);
      expect(controller.bulkSelectedIds, {'one', 'two', 'three'});
      controller.updateBulkDragSelection('one', orderedIds: order);
      expect(controller.bulkSelectedIds, {'one', 'two'});
      expect(await controller.deleteBulkSelection(), isFalse);
      controller.endBulkDragSelection();
      expect(controller.isBulkDragSelecting, isFalse);
      expect(controller.isBulkSelecting, isTrue);
      expect(controller.canSuspend, isTrue);
    },
  );

  test('held selection defers live refresh until release', () async {
    controller.beginBulkDragSelection('one');
    final reads = repository.queryCalls;
    repository.clips.removeWhere((clip) => clip.id == 'one');
    repository.events.add(HistoryRuntimeEvent.itemsChanged);
    await Future<void>.delayed(Duration.zero);
    expect(repository.queryCalls, reads);
    expect(controller.bulkSelectedIds, {'one'});
    controller.endBulkDragSelection();
    await Future<void>.delayed(Duration.zero);
    expect(repository.queryCalls, greaterThan(reads));
    expect(controller.bulkSelectedIds, isEmpty);
  });

  test(
    'entry clears the inspector and taps only toggle bulk membership',
    () async {
      await controller.select('one');
      controller.beginBulkSelection('one');
      expect(controller.isBulkSelecting, isTrue);
      expect(controller.bulkSelectedIds, {'one'});
      expect(controller.selectedClip, isNull);
      controller.toggleBulkSelection('two');
      await controller.select('two');
      expect(controller.selectedId, isNull);
      expect(controller.bulkSelectedIds, {'one', 'two'});
      controller.toggleBulkSelection('one');
      controller.toggleBulkSelection('two');
      expect(controller.isBulkSelecting, isTrue);
      expect(controller.bulkSelectedIds, isEmpty);
      controller.endBulkSelection();
      expect(controller.isBulkSelecting, isFalse);
      await controller.select('two');
      expect(controller.selectedId, 'two');
    },
  );

  test(
    'selection spans loaded pages and resets when the query changes',
    () async {
      controller.beginBulkSelection('one');
      controller.toggleBulkSelection('three');
      expect(controller.bulkSelectedIds, {'one'});
      await controller.loadMore();
      controller.toggleBulkSelection('three');
      expect(controller.bulkSelectedIds, {'one', 'three'});
      await controller.updateQuery(const HistoryQuery(pinnedOnly: true));
      expect(controller.isBulkSelecting, isFalse);
      expect(controller.bulkSelectedIds, isEmpty);
      expect(controller.items.map((clip) => clip.id), ['one']);
    },
  );

  test(
    'explicit pin and unpin apply to mixed selections and keep them selected',
    () async {
      controller.beginBulkSelection('one');
      controller.toggleBulkSelection('two');
      expect(await controller.setBulkPinned(true), isTrue);
      expect(repository.pinUpdates, [('two', true)]);
      expect(controller.bulkSelectedIds, {'one', 'two'});
      expect(controller.items.every((clip) => clip.pinned), isTrue);
      expect(await controller.setBulkPinned(false), isTrue);
      expect(repository.pinUpdates, [
        ('two', true),
        ('one', false),
        ('two', false),
      ]);
      expect(controller.items.every((clip) => !clip.pinned), isTrue);
    },
  );

  test(
    'deletes only chosen clips, including pins, and keeps failures retryable',
    () async {
      repository.failedDeletes.add('two');
      controller.beginBulkSelection('one');
      controller.toggleBulkSelection('two');
      expect(await controller.deleteBulkSelection(), isFalse);
      expect(repository.deletedIds, ['one']);
      expect(repository.clips.map((clip) => clip.id), ['two', 'three']);
      expect(controller.bulkSelectedIds, {'two'});
      expect(controller.isBulkSelecting, isTrue);
      expect(controller.errorMessage, contains('could not be deleted'));
      repository.failedDeletes.clear();
      expect(await controller.deleteBulkSelection(), isTrue);
      expect(repository.deletedIds, ['one', 'two']);
      expect(repository.clips.single.id, 'three');
      expect(controller.isBulkSelecting, isFalse);
      expect(controller.bulkSelectedIds, isEmpty);
    },
  );

  test(
    'failed pins preserve actual state and report partial failure',
    () async {
      await controller.loadMore();
      repository.failedPins.add('three');
      controller.beginBulkSelection('two');
      controller.toggleBulkSelection('three');
      expect(await controller.setBulkPinned(true), isFalse);
      expect(
        repository.clips.firstWhere((clip) => clip.id == 'two').pinned,
        isTrue,
      );
      expect(
        repository.clips.firstWhere((clip) => clip.id == 'three').pinned,
        isFalse,
      );
      expect(controller.bulkSelectedIds, {'two', 'three'});
      expect(controller.errorMessage, contains('could not be updated'));
    },
  );

  test(
    'deleting the final selected clips exits into the empty state',
    () async {
      await controller.loadMore();
      controller.beginBulkSelection();
      for (final clip in controller.items) {
        controller.toggleBulkSelection(clip.id);
      }
      expect(await controller.deleteBulkSelection(), isTrue);
      expect(controller.state, HistoryLoadState.empty);
      expect(controller.items, isEmpty);
      expect(controller.isBulkSelecting, isFalse);
      expect(controller.isBulkMutating, isFalse);
      expect(controller.canLoadMore, isFalse);
    },
  );

  test(
    'locks the selection and competing mutations until a batch completes',
    () async {
      final pending = Completer<void>();
      repository.mutationPending = pending.future;
      controller.beginBulkSelection('one');
      final batch = controller.deleteBulkSelection();
      expect(controller.isBulkMutating, isTrue);
      expect(controller.canSuspend, isFalse);
      expect(controller.canReorderPinned, isFalse);
      controller.endBulkSelection();
      controller.toggleBulkSelection('two');
      await controller.updateQuery(const HistoryQuery(search: 'other'));
      expect(await controller.deleteBulkSelection(), isFalse);
      expect(await controller.setBulkPinned(true), isFalse);
      expect(await controller.deleteClip('two'), isFalse);
      expect(await controller.togglePin(controller.items.last), isFalse);
      expect(await controller.deleteAll(), isFalse);
      expect(controller.bulkSelectedIds, {'one'});
      expect(controller.query.search, isEmpty);
      pending.complete();
      expect(await batch, isTrue);
      expect(controller.isBulkMutating, isFalse);
    },
  );

  test(
    'live events defer refresh until the batch and rebuild pagination',
    () async {
      repository.emitMutations = true;
      controller.beginBulkSelection('one');
      controller.toggleBulkSelection('two');
      final reads = repository.queryCalls;
      expect(await controller.deleteBulkSelection(), isTrue);
      expect(repository.readsDuringDelete, [reads, reads]);
      expect(controller.items.single.id, 'three');
      expect(controller.canLoadMore, isFalse);
    },
  );

  test(
    'live refresh prunes removed IDs without selecting incoming clips',
    () async {
      controller.beginBulkSelection('one');
      controller.toggleBulkSelection('two');
      repository.clips.removeWhere((clip) => clip.id == 'one');
      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);
      expect(controller.bulkSelectedIds, {'two'});
      expect(controller.items.map((clip) => clip.id), ['two', 'three']);
    },
  );
}

class _BulkRepository implements HistoryRepository {
  final events = StreamController<HistoryRuntimeEvent>.broadcast();
  final clips = [
    for (final id in ['one', 'two', 'three'])
      HistoryClip(
        id: id,
        contentType: 'text',
        preview: id,
        createdAt: DateTime.utc(2026),
        pinned: id == 'one',
      ),
  ];
  final pinUpdates = <(String, bool)>[];
  final deletedIds = <String>[];
  final readsDuringDelete = <int>[];
  final failedDeletes = <String>{};
  final failedPins = <String>{};
  Future<void>? mutationPending;
  bool emitMutations = false;
  int queryCalls = 0;

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
    queryCalls++;
    final matching = clips
        .where((clip) => !query.pinnedOnly || clip.pinned)
        .toList();
    final start = int.parse(cursor ?? '0');
    final items = matching.skip(start).take(limit).toList();
    final end = start + items.length;
    return HistoryClipPage(
      items: items,
      nextCursor: end < matching.length ? '$end' : null,
    );
  }

  @override
  Future<HistoryClip> get(String id) async =>
      clips.firstWhere((clip) => clip.id == id);
  @override
  Future<void> delete(String id) async {
    readsDuringDelete.add(queryCalls);
    await mutationPending;
    if (failedDeletes.contains(id)) throw StateError('offline');
    deletedIds.add(id);
    clips.removeWhere((clip) => clip.id == id);
    if (emitMutations) events.add(HistoryRuntimeEvent.itemsChanged);
  }

  @override
  Future<void> setPinned(String id, bool pinned) async {
    await mutationPending;
    if (failedPins.contains(id)) throw StateError('offline');
    pinUpdates.add((id, pinned));
    final index = clips.indexWhere((clip) => clip.id == id);
    clips[index] = clips[index].copyWith(pinned: pinned);
    if (emitMutations) events.add(HistoryRuntimeEvent.itemsChanged);
  }

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
  Future<void> importFile(HistoryImportFile file) async {}
  @override
  Future<void> deleteAll() async {}
  @override
  Future<void> reorderPinned(List<String> ids) async {}
}
