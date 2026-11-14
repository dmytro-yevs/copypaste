import 'dart:async';

import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/paste_queue/models/paste_queue_models.dart';
import 'package:copypaste_flutter/features/paste_queue/repository/paste_queue_repository.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';

/// An in-memory queue that records every call in order.
class FakePasteQueueRepository implements PasteQueueRepository {
  FakePasteQueueRepository({int entries = 0, this.collecting = false}) {
    for (var index = 1; index <= entries; index++) {
      _entries.add(_Entry('e$index', 'value $index'));
    }
  }

  final calls = <String>[];
  final _changes = StreamController<int?>.broadcast();
  final _entries = <_Entry>[];
  bool collecting;
  int _revision = 1;
  int _tokens = 0;
  final _inFlight = <String, List<String>>{};

  /// When set, each delivery waits for it before answering.
  Completer<void>? holdDelivery;

  List<String> get statuses => [
    for (final entry in _entries) entry.status.name,
  ];

  @override
  Future<PasteQueueSnapshot> state() async => _snapshot();

  PasteQueueSnapshot _snapshot() {
    final pending = _entries.any(
      (entry) => entry.status == PasteQueueEntryStatus.pending,
    );
    return PasteQueueSnapshot(
      revision: _revision,
      collecting: collecting,
      phase: _entries.isEmpty
          ? PasteQueuePhase.empty
          : pending
          ? PasteQueuePhase.active
          : PasteQueuePhase.finished,
      entries: [
        for (final (index, entry) in _entries.indexed)
          PasteQueueEntry(
            id: entry.id,
            position: '${index + 1}',
            status: entry.status,
            kind: PasteQueueContentKind.text,
            preview: entry.text,
          ),
      ],
    );
  }

  PasteQueueSnapshot _changed() {
    _revision += 1;
    return _snapshot();
  }

  @override
  Future<PasteQueueSnapshot> setCollecting(
    bool on, {
    bool append = false,
  }) async {
    calls.add('collecting:$on');
    collecting = on;
    return _changed();
  }

  @override
  Future<PasteQueueDelivery> deliver(
    PasteQueueTarget target, {
    String? entryId,
    bool? plainText,
    required PasteQueueDeliveryMode mode,
  }) async {
    final next = _entries
        .where((entry) => entry.status == PasteQueueEntryStatus.pending)
        .firstOrNull;
    calls.add('deliver:${next?.id ?? '-'}');
    await holdDelivery?.future;
    if (next == null) {
      return const PasteQueueDelivery(nothing: PasteQueueNothing.empty);
    }
    next.status = PasteQueueEntryStatus.pasted;
    _changed();
    final last = !_entries.any(
      (entry) => entry.status == PasteQueueEntryStatus.pending,
    );
    if (mode == PasteQueueDeliveryMode.copyOnly) {
      return PasteQueueDelivery(entryIds: [next.id], finished: last);
    }
    final token = 't${++_tokens}';
    _inFlight[token] = [next.id];
    return PasteQueueDelivery(
      token: token,
      entryIds: [next.id],
      finished: last,
    );
  }

  @override
  Future<PasteQueueSnapshot> confirm(
    String token, {
    required bool inserted,
  }) async {
    calls.add('confirm:$token:$inserted');
    final ids = _inFlight.remove(token) ?? const [];
    if (!inserted) {
      for (final entry in _entries) {
        if (ids.contains(entry.id)) {
          entry.status = PasteQueueEntryStatus.pending;
        }
      }
    }
    return _changed();
  }

  @override
  Future<PasteQueueSnapshot> add(List<PasteQueueSource> sources) async =>
      _changed();

  @override
  Future<PasteQueueSnapshot> remove(List<String> entryIds) async {
    _entries.removeWhere((entry) => entryIds.contains(entry.id));
    return _changed();
  }

  @override
  Future<PasteQueueSnapshot> removeFinished() async => _changed();

  @override
  Future<PasteQueueSnapshot> clear() async {
    _entries.clear();
    return _changed();
  }

  @override
  Future<PasteQueueSnapshot> undoClear() async => _changed();

  @override
  Future<PasteQueueSnapshot> reverse() async => _changed();

  @override
  Future<PasteQueueSnapshot> move(String entryId, int toIndex) async =>
      _changed();

  @override
  Future<PasteQueueSnapshot> edit(String entryId, String? text) async =>
      _changed();

  @override
  Future<PasteQueueSnapshot> stepBack() async => _changed();

  @override
  Future<PasteQueueSnapshot> skip() async => _changed();

  @override
  Future<PasteQueueSettings> setSettings(PasteQueueSettings settings) async =>
      settings;

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String iconId) async => null;

  @override
  Stream<int?> changes() => _changes.stream;
}

class _Entry {
  _Entry(this.id, this.text);

  final String id;
  final String text;
  PasteQueueEntryStatus status = PasteQueueEntryStatus.pending;
}

/// Answers every paste with [outcome] and records it in [calls].
class FakeForegroundPasteHost implements ForegroundPasteHost {
  FakeForegroundPasteHost({
    this.outcome = PasteOutcome.pasted,
    this.accessibility = true,
    List<String>? calls,
  }) : calls = calls ?? [];

  PasteOutcome outcome;
  bool accessibility;
  final List<String> calls;

  @override
  Future<PasteOutcome> pasteForeground({bool plainText = false}) async {
    calls.add('paste');
    return outcome;
  }

  @override
  Future<bool> accessibilityGranted() async => accessibility;

  @override
  Future<bool> moveCaretLeft(int count) async => true;

  @override
  Future<bool?> secureFieldFocused() async => null;
}
