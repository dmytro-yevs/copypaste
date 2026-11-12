import 'dart:async';

import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_group_counts_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_scope.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/repository/runtime_history_repository.dart';
import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';

final _today = DateTime(2026, 11, 11);
final _yesterday = DateTime(2026, 11, 10);

HistoryClip _clip(String id, DateTime createdAt, {bool pinned = false}) =>
    HistoryClip(
      id: id,
      contentType: 'text/plain',
      preview: id,
      createdAt: createdAt,
      pinned: pinned,
    );

class _Repository extends Fake
    implements HistoryRepository, HistoryGroupCountsRepository {
  final events = StreamController<HistoryRuntimeEvent>.broadcast();
  final countRequests = <(HistoryQuery, List<DateTime>)>[];
  HistoryGroupCounts? Function(List<DateTime> days) answer = (_) => null;
  int skippedUndecryptable = 0;
  var clips = <HistoryClip>[
    _clip('pin', DateTime(2026, 1, 1), pinned: true),
    _clip('a', _today.add(const Duration(hours: 9))),
    _clip('b', _today.add(const Duration(hours: 8))),
    _clip('c', _yesterday.add(const Duration(hours: 23))),
  ];

  @override
  Stream<HistoryRuntimeEvent> watch() => events.stream;

  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async => HistoryClipPage(
    items: List.of(clips),
    skippedUndecryptable: skippedUndecryptable,
  );

  @override
  Future<void> delete(String id) async =>
      clips.removeWhere((clip) => clip.id == id);

  @override
  Future<void> deleteAll() async => clips.removeWhere((clip) => !clip.pinned);

  @override
  Future<HistoryGroupCounts?> groupCounts({
    required HistoryQuery query,
    required List<DateTime> days,
  }) async {
    countRequests.add((query, days));
    return answer(days);
  }
}

Future<void> _settle() async {
  for (var i = 0; i < 20; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

void main() {
  test('local days map to contiguous instants across clock changes', () {
    final days = [for (var i = 0; i < 366; i++) DateTime(2026, 1, 1 + i)];
    final ranges = RuntimeHistoryRepository.mapDayRanges(days);
    for (var i = 0; i < days.length; i++) {
      final from = ranges[i].fromMs.toInt();
      final before = ranges[i].beforeMs.toInt();
      expect(from, days[i].millisecondsSinceEpoch);
      expect(
        (before - from) / Duration.millisecondsPerHour,
        inInclusiveRange(23, 25),
        reason: 'A local day keeps its real length on ${days[i]}',
      );
      if (i > 0) {
        expect(from, ranges[i - 1].beforeMs.toInt(), reason: '${days[i]}');
      }
    }

    final counts = RuntimeHistoryRepository.mapGroupCounts(
      [_today, _yesterday],
      runtime.ClipGroupCounts(
        pinned: BigInt.from(2),
        ranges: Uint64List.fromList([14, 3]),
      ),
    );
    expect(counts.pinned, 2);
    expect(counts.day(_today.add(const Duration(hours: 13))), 14);
    expect(counts.day(_yesterday), 3);
    expect(counts.day(DateTime(2026, 11, 9)), isNull);
    expect(
      () => RuntimeHistoryRepository.mapGroupCounts(
        [_today],
        runtime.ClipGroupCounts(
          pinned: BigInt.zero,
          ranges: Uint64List.fromList([1, 2]),
        ),
      ),
      throwsStateError,
    );
  });

  test('counts follow the displayed days, the history and the query', () async {
    final repo = _Repository()
      ..answer = (days) => HistoryGroupCounts(
        pinned: 1,
        days: {for (final day in days) day: day == _today ? 14 : 3},
      );
    final controller = HistoryController(
      repo,
      includeFacets: false,
      includeGroupCounts: true,
    );
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    var notified = 0;
    controller.addListener(() => notified++);
    await controller.initialize();
    await _settle();

    expect(repo.countRequests, hasLength(1));
    expect(repo.countRequests.single.$2, [_today, _yesterday]);
    final counts = controller.window.groupCounts!;
    expect(counts.pinned, 1);
    expect(counts.day(_today), 14);
    expect(counts.day(_yesterday), 3);

    // The visible rows are unchanged, yet a day can have gained a clip
    // beyond them: a history change always re-reads the counts.
    repo.answer = (days) =>
        HistoryGroupCounts(pinned: 1, days: {for (final day in days) day: 15});
    final before = notified;
    repo.events.add(HistoryRuntimeEvent.itemsChanged);
    await _settle();
    expect(repo.countRequests, hasLength(2));
    expect(controller.window.groupCounts!.day(_today), 15);
    expect(notified, greaterThan(before));

    // A filter the runtime cannot count is asked about once, not per change.
    repo.answer = (_) => null;
    await controller.window.updateQuery(
      const HistoryQuery(kind: HistoryClipKind.image),
    );
    await _settle();
    expect(repo.countRequests, hasLength(3));
    expect(repo.countRequests.last.$1.kind, HistoryClipKind.image);
    expect(controller.window.groupCounts, isNull);
    repo.events.add(HistoryRuntimeEvent.itemsChanged);
    await _settle();
    expect(repo.countRequests, hasLength(3));

    // Relevance order shows no day groups and always carries a search.
    await controller.window.updateQuery(
      const HistoryQuery(search: 'needle', sort: HistorySort.relevance),
    );
    await _settle();
    expect(repo.countRequests, hasLength(3));
    expect(controller.window.groupCounts, isNull);
  });

  test('counts are withheld when they could overstate the list', () async {
    final repo = _Repository()
      ..skippedUndecryptable = 1
      ..answer = (days) => HistoryGroupCounts(pinned: 1, days: const {});
    final controller = HistoryController(
      repo,
      includeFacets: false,
      includeGroupCounts: true,
    );
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    await controller.initialize();
    await _settle();
    expect(repo.countRequests, isEmpty);
    expect(controller.window.groupCounts, isNull);

    final plain = _Repository()
      ..answer = (days) => HistoryGroupCounts(pinned: 1, days: const {});
    final unrequested = HistoryController(plain, includeFacets: false);
    addTearDown(unrequested.dispose);
    addTearDown(plain.events.close);
    await unrequested.initialize();
    await _settle();
    expect(
      plain.countRequests,
      isEmpty,
      reason: 'A presentation without group headers must not ask for counts',
    );
  });

  test('deleting re-reads the counts without a history event', () async {
    final repo = _Repository()
      ..answer = (days) =>
          HistoryGroupCounts(pinned: 1, days: {for (final day in days) day: 2});
    final controller = HistoryController(
      repo,
      includeFacets: false,
      includeGroupCounts: true,
    );
    addTearDown(controller.dispose);
    addTearDown(repo.events.close);
    await controller.initialize();
    await _settle();
    expect(repo.countRequests, hasLength(1));

    repo.answer = (days) =>
        HistoryGroupCounts(pinned: 1, days: {for (final day in days) day: 1});
    expect(await controller.mutations.deleteClip('b'), isTrue);
    await _settle();
    expect(repo.countRequests, hasLength(2));
    expect(controller.window.groupCounts!.day(_today), 1);

    repo.answer = (_) => HistoryGroupCounts(pinned: 1, days: const {});
    expect(await controller.mutations.deleteAll(), isTrue);
    await _settle();
    expect(repo.countRequests, hasLength(3));
    expect(controller.window.groupCounts!.day(_today), isNull);
  });

  test('a failed count is asked again only after a change', () async {
    final repo = _Repository()
      ..answer = (_) => throw StateError('counts unavailable');
    addTearDown(repo.events.close);
    final counts = HistoryGroupCountsController(
      HistoryScope(repo, () {}),
      enabled: true,
    );
    const query = HistoryQuery();
    void publish({bool historyChanged = false}) => counts.sync(
      query: query,
      days: [_today],
      listed: true,
      exact: true,
      historyChanged: historyChanged,
    );

    publish();
    await _settle();
    expect(repo.countRequests, hasLength(1));
    expect(counts.counts, isNull);
    publish();
    publish();
    await _settle();
    expect(repo.countRequests, hasLength(1));

    publish(historyChanged: true);
    await _settle();
    expect(repo.countRequests, hasLength(2));
  });
}
