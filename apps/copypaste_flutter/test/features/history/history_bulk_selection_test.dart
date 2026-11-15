import 'dart:async';

import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_window_controller.dart';
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
      await controller.window.loadMore();
      await controller.selection.select('one');
      const order = ['one', 'three', 'two'];
      controller.bulk.selectBulkClip('two', orderedIds: order, range: true);
      expect(controller.bulk.bulkSelectedIds, {'one', 'three', 'two'});
      expect(controller.selection.selectedId, isNull);
      controller.bulk.selectBulkClip('three', orderedIds: order, range: true);
      expect(controller.bulk.bulkSelectedIds, {'one', 'three'});
    },
  );

  test(
    'Ctrl adds to the inspector selection and Ctrl Shift preserves other clips',
    () async {
      await controller.window.loadMore();
      await controller.selection.select('one');
      const order = ['one', 'two', 'three'];
      controller.bulk.selectBulkClip(
        'three',
        orderedIds: order,
        additive: true,
      );
      expect(controller.bulk.bulkSelectedIds, {'one', 'three'});
      controller.bulk.selectBulkClip(
        'two',
        orderedIds: order,
        additive: true,
        range: true,
      );
      expect(controller.bulk.bulkSelectedIds, {'one', 'two', 'three'});
      controller.bulk.selectBulkClip('one', orderedIds: order, additive: true);
      expect(controller.bulk.bulkSelectedIds, {'two', 'three'});
    },
  );

  test(
    'held range reverses without toggling clips or losing its original selection',
    () async {
      controller.bulk.beginBulkSelection('two');
      expect(controller.bulk.beginBulkDragSelection('one'), isTrue);
      expect(controller.canSuspend, isFalse);
      await controller.window.loadMore();
      const order = ['one', 'two', 'three'];
      controller.bulk.updateBulkDragSelection('three', orderedIds: order);
      expect(controller.bulk.bulkSelectedIds, {'one', 'two', 'three'});
      controller.bulk.updateBulkDragSelection('one', orderedIds: order);
      expect(controller.bulk.bulkSelectedIds, {'one', 'two'});
      expect(await controller.bulk.deleteBulkSelection(), isFalse);
      controller.bulk.endBulkDragSelection();
      expect(controller.bulk.isBulkDragSelecting, isFalse);
      expect(controller.bulk.isBulkSelecting, isTrue);
      expect(controller.canSuspend, isTrue);
    },
  );

  test('held selection defers live refresh until release', () async {
    controller.bulk.beginBulkDragSelection('one');
    final reads = repository.queryCalls;
    repository.clips.removeWhere((clip) => clip.id == 'one');
    repository.events.add(HistoryRuntimeEvent.itemsChanged);
    await Future<void>.delayed(Duration.zero);
    expect(repository.queryCalls, reads);
    expect(controller.bulk.bulkSelectedIds, {'one'});
    controller.bulk.endBulkDragSelection();
    await Future<void>.delayed(Duration.zero);
    expect(repository.queryCalls, greaterThan(reads));
    expect(controller.bulk.bulkSelectedIds, isEmpty);
  });

  test(
    'entry clears the inspector and taps only toggle bulk membership',
    () async {
      await controller.selection.select('one');
      controller.bulk.beginBulkSelection('one');
      expect(controller.bulk.isBulkSelecting, isTrue);
      expect(controller.bulk.bulkSelectedIds, {'one'});
      expect(controller.selection.selectedClip, isNull);
      controller.bulk.toggleBulkSelection('two');
      await controller.selection.select('two');
      expect(controller.selection.selectedId, isNull);
      expect(controller.bulk.bulkSelectedIds, {'one', 'two'});
      controller.bulk.toggleBulkSelection('one');
      controller.bulk.toggleBulkSelection('two');
      expect(controller.bulk.isBulkSelecting, isTrue);
      expect(controller.bulk.bulkSelectedIds, isEmpty);
      controller.bulk.endBulkSelection();
      expect(controller.bulk.isBulkSelecting, isFalse);
      await controller.selection.select('two');
      expect(controller.selection.selectedId, 'two');
    },
  );

  test(
    'selection spans loaded pages and resets when the query changes',
    () async {
      controller.bulk.beginBulkSelection('one');
      controller.bulk.toggleBulkSelection('three');
      expect(controller.bulk.bulkSelectedIds, {'one'});
      await controller.window.loadMore();
      controller.bulk.toggleBulkSelection('three');
      expect(controller.bulk.bulkSelectedIds, {'one', 'three'});
      await controller.window.updateQuery(const HistoryQuery(pinnedOnly: true));
      expect(controller.bulk.isBulkSelecting, isFalse);
      expect(controller.bulk.bulkSelectedIds, isEmpty);
      expect(controller.window.items.map((clip) => clip.id), ['one']);
    },
  );

  test(
    'explicit pin and unpin apply to mixed selections and keep them selected',
    () async {
      controller.bulk.beginBulkSelection('one');
      controller.bulk.toggleBulkSelection('two');
      expect(await controller.bulk.setBulkPinned(true), isTrue);
      expect(repository.pinUpdates, [('two', true)]);
      expect(controller.bulk.bulkSelectedIds, {'one', 'two'});
      expect(controller.window.items.every((clip) => clip.pinned), isTrue);
      expect(await controller.bulk.setBulkPinned(false), isTrue);
      expect(repository.pinUpdates, [
        ('two', true),
        ('one', false),
        ('two', false),
      ]);
      expect(controller.window.items.every((clip) => !clip.pinned), isTrue);
    },
  );

  test(
    'deletes only chosen clips, including pins, and keeps failures retryable',
    () async {
      repository.failedDeletes.add('two');
      controller.bulk.beginBulkSelection('one');
      controller.bulk.toggleBulkSelection('two');
      expect(await controller.bulk.deleteBulkSelection(), isFalse);
      expect(repository.deletedIds, ['one']);
      expect(repository.clips.map((clip) => clip.id), ['two', 'three']);
      expect(controller.bulk.bulkSelectedIds, {'two'});
      expect(controller.bulk.isBulkSelecting, isTrue);
      expect(controller.errors.message, contains('could not be deleted'));
      repository.failedDeletes.clear();
      expect(await controller.bulk.deleteBulkSelection(), isTrue);
      expect(repository.deletedIds, ['one', 'two']);
      expect(repository.clips.single.id, 'three');
      expect(controller.bulk.isBulkSelecting, isFalse);
      expect(controller.bulk.bulkSelectedIds, isEmpty);
    },
  );

  test(
    'failed pins preserve actual state and report partial failure',
    () async {
      await controller.window.loadMore();
      repository.failedPins.add('three');
      controller.bulk.beginBulkSelection('two');
      controller.bulk.toggleBulkSelection('three');
      expect(await controller.bulk.setBulkPinned(true), isFalse);
      expect(
        repository.clips.firstWhere((clip) => clip.id == 'two').pinned,
        isTrue,
      );
      expect(
        repository.clips.firstWhere((clip) => clip.id == 'three').pinned,
        isFalse,
      );
      expect(controller.bulk.bulkSelectedIds, {'two', 'three'});
      expect(controller.errors.message, contains('could not be updated'));
    },
  );

  test(
    'deleting the final selected clips exits into the empty state',
    () async {
      await controller.window.loadMore();
      controller.bulk.beginBulkSelection();
      for (final clip in controller.window.items) {
        controller.bulk.toggleBulkSelection(clip.id);
      }
      expect(await controller.bulk.deleteBulkSelection(), isTrue);
      expect(controller.window.state, HistoryLoadState.empty);
      expect(controller.window.items, isEmpty);
      expect(controller.bulk.isBulkSelecting, isFalse);
      expect(controller.bulk.isBulkMutating, isFalse);
      expect(controller.window.canLoadMore, isFalse);
    },
  );

  test(
    'locks the selection and competing mutations until a batch completes',
    () async {
      final pending = Completer<void>();
      repository.mutationPending = pending.future;
      controller.bulk.beginBulkSelection('one');
      final batch = controller.bulk.deleteBulkSelection();
      expect(controller.bulk.isBulkMutating, isTrue);
      expect(controller.canSuspend, isFalse);
      expect(controller.pinnedOrder.canReorderPinned, isFalse);
      controller.bulk.endBulkSelection();
      controller.bulk.toggleBulkSelection('two');
      await controller.window.updateQuery(const HistoryQuery(search: 'other'));
      expect(await controller.bulk.deleteBulkSelection(), isFalse);
      expect(await controller.bulk.setBulkPinned(true), isFalse);
      expect(await controller.mutations.deleteClip('two'), isFalse);
      expect(
        await controller.mutations.togglePin(controller.window.items.last),
        isFalse,
      );
      expect(await controller.mutations.deleteAll(), isFalse);
      expect(controller.bulk.bulkSelectedIds, {'one'});
      expect(controller.window.query.search, isEmpty);
      pending.complete();
      expect(await batch, isTrue);
      expect(controller.bulk.isBulkMutating, isFalse);
    },
  );

  test(
    'live events defer refresh until the batch and rebuild pagination',
    () async {
      repository.emitMutations = true;
      controller.bulk.beginBulkSelection('one');
      controller.bulk.toggleBulkSelection('two');
      final reads = repository.queryCalls;
      expect(await controller.bulk.deleteBulkSelection(), isTrue);
      expect(repository.readsDuringDelete, [reads, reads]);
      expect(controller.window.items.single.id, 'three');
      expect(controller.window.canLoadMore, isFalse);
    },
  );

  test(
    'live refresh prunes removed IDs without selecting incoming clips',
    () async {
      controller.bulk.beginBulkSelection('one');
      controller.bulk.toggleBulkSelection('two');
      repository.clips.removeWhere((clip) => clip.id == 'one');
      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);
      expect(controller.bulk.bulkSelectedIds, {'two'});
      expect(controller.window.items.map((clip) => clip.id), ['two', 'three']);
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
  Future<HistoryClip> setSecret(String id, bool secret) async =>
      throw UnimplementedError('setSecret');

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
  Future<void> copyText(String id, String text) async {}

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
