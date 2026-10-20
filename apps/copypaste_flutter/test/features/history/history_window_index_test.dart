import 'package:copypaste_flutter/features/history/controller/history_window_index.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:flutter_test/flutter_test.dart';

HistoryClip _clip(String id, {String? preview}) => HistoryClip(
  id: id,
  contentType: 'text/plain',
  preview: preview ?? id,
  createdAt: DateTime.utc(2026),
  pinned: false,
);

void main() {
  test('retains cursor and ids after visiting 10000 rows', () {
    final index = HistoryWindowIndex(maxPages: 3, maxBytes: 1000000);
    for (var page = 0; page < 200; page++) {
      index.append(
        page == 0 ? null : 'cursor-$page',
        HistoryClipPage(
          items: [
            for (var row = 0; row < 50; row++) _clip('${page * 50 + row}'),
          ],
          nextCursor: 'cursor-${page + 1}',
        ),
      );
      index.pages.last.extent = 100;
      index.retainAround(page);
    }
    expect(index.ids.length, 10000);
    expect(index.residentPageCount, 3);
    expect(index.pages[3].items, isNull);
    expect(index.pages[3].inputCursor, 'cursor-3');
    expect(index.pages[3].ids.first, '150');
  });

  test('preserves an oversized active page and evicts it later', () {
    final index = HistoryWindowIndex(maxPages: 3, maxBytes: 30);
    index.append(
      null,
      HistoryClipPage(
        items: [_clip('a', preview: 'x' * 100)],
        nextCursor: 'one',
      ),
    );
    index.pages.last.extent = 100;
    index.retainAround(0);
    expect(index.pages.first.items, isNotNull);
    expect(index.residentBytes, greaterThan(index.maxBytes));
    index.append('one', HistoryClipPage(items: [_clip('b')]));
    index.pages.last.extent = 100;
    index.retainAround(1);
    expect(index.pages.first.items, isNull);
    expect(index.residentBytes, lessThanOrEqualTo(index.maxBytes));
  });

  test('keeps separate bounded pinned and recent page windows', () {
    final index = HistoryWindowIndex(
      maxPages: 3,
      maxBytes: 100000,
      independentPins: true,
    );
    for (var page = 0; page < 10; page++) {
      final clip = _clip('$page').copyWith(pinned: page < 5);
      index.append(
        page == 0 ? null : '$page',
        HistoryClipPage(items: [clip], nextCursor: '${page + 1}'),
      );
      index.pages.last.extent = 100;
      index.retainAround(page);
    }
    expect(index.residentPageCount, 6);
    expect(
      index.pages.where(
        (page) => page.items != null && page.pinnedIds.isNotEmpty,
      ),
      hasLength(3),
    );
    expect(
      index.pages.where((page) => page.items != null && page.pinnedIds.isEmpty),
      hasLength(3),
    );
  });

  test('evicts recent rows from a mixed page without losing pinned rows', () {
    final index = HistoryWindowIndex(
      maxPages: 2,
      maxBytes: 100000,
      independentPins: true,
    );
    index.append(
      null,
      HistoryClipPage(
        items: [_clip('pin').copyWith(pinned: true), _clip('recent-0')],
        nextCursor: 'one',
      ),
    );
    index.pages.last.extent = 100;
    index.append(
      'one',
      HistoryClipPage(items: [_clip('recent-1')], nextCursor: 'two'),
    );
    index.pages.last.extent = 100;
    index.append('two', HistoryClipPage(items: [_clip('recent-2')]));
    index.pages.last.extent = 100;
    index.retainAround(2, protectedIds: {'pin'});
    expect(index.pages.first.items!.map((clip) => clip.id), ['pin']);
    expect(index.pages.first.ids, ['pin', 'recent-0']);
    expect(index.pages[1].items, isNotNull);
    expect(index.pages[2].items, isNotNull);
  });

  test('mixed-page updates preserve evicted pin metadata', () {
    final index = HistoryWindowIndex(
      maxPages: 1,
      maxBytes: 100000,
      independentPins: true,
    );
    index.append(
      null,
      HistoryClipPage(
        items: [_clip('pin').copyWith(pinned: true), _clip('recent')],
        nextCursor: 'next',
      ),
    );
    index.append(
      'next',
      HistoryClipPage(items: [_clip('other-pin').copyWith(pinned: true)]),
    );
    index.retainAround(1, protectedIds: {'recent'});
    expect(index.pages.first.items!.map((clip) => clip.id), ['recent']);

    index.replaceItem(_clip('recent', preview: 'updated'));
    expect(index.pages.first.pinnedIds, ['pin']);
    expect(index.pinnedIds, ['pin', 'other-pin']);
    expect(index.pages.first.items!.single.preview, 'updated');

    index.replaceItem(_clip('pin'));
    expect(index.pages.first.pinnedIds, isEmpty);
    expect(index.pages.first.items!.map((clip) => clip.id), ['recent']);
  });

  test('changed cursor invalidates later saved page references', () {
    final index = HistoryWindowIndex(maxPages: 3, maxBytes: 1000);
    index.append(null, HistoryClipPage(items: [_clip('a')], nextCursor: 'old'));
    index.append('old', HistoryClipPage(items: [_clip('b')]));
    index.replace(0, HistoryClipPage(items: [_clip('a')], nextCursor: 'new'));
    expect(index.pages, hasLength(1));
    expect(index.pages.single.nextCursor, 'new');
  });
}
