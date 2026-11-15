import 'dart:async';

import 'package:copypaste_flutter/features/history/controller/history_spoiler_controller.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('reconnection rejects a prior runtime response', () async {
    final repository = _Repository();
    final controller = HistorySpoilerController(repository);
    addTearDown(controller.dispose);
    await controller.setRevealed('old', true);
    final old = Completer<HistorySpoilerState>();
    repository.pending = old;
    final staleRead = controller.refresh();
    controller.disconnected();
    repository.pending = null;
    repository.state = const HistorySpoilerState(0, {});
    await controller.refresh();
    await controller.setRevealed('new', true);
    old.complete(const HistorySpoilerState(99, {'old'}));
    await staleRead;
    expect(controller.isRevealed('old'), isFalse);
    expect(controller.isRevealed('new'), isTrue);
  });

  test('two local clients share reveals and conceal every surface', () async {
    final repository = _Repository();
    final history = HistorySpoilerController(repository);
    final quickPaste = HistorySpoilerController(repository);
    addTearDown(history.dispose);
    addTearDown(quickPaste.dispose);
    await history.refresh();
    await quickPaste.refresh();
    await history.setRevealed('clip', true);
    await quickPaste.refresh();
    expect(quickPaste.isRevealed('clip'), isTrue);
    await quickPaste.setRevealed(null, false);
    await history.refresh();
    expect(history.isRevealed('clip'), isFalse);
  });

  test('a delayed read cannot undo a newer conceal', () async {
    final repository = _Repository();
    final controller = HistorySpoilerController(repository);
    addTearDown(controller.dispose);
    await controller.setRevealed('clip', true);
    final stale = repository.state;
    final read = Completer<HistorySpoilerState>();
    repository.pending = read;
    final refresh = controller.refresh();
    await controller.setRevealed(null, false);
    read.complete(stale);
    await refresh;
    expect(controller.isRevealed('clip'), isFalse);
  });

  test('refreshes during a read share one follow-up read', () async {
    final repository = _Repository();
    final controller = HistorySpoilerController(repository);
    addTearDown(controller.dispose);
    final first = Completer<HistorySpoilerState>();
    repository.pending = first;
    final refreshes = [
      controller.refresh(),
      controller.refresh(),
      controller.refresh(),
    ];
    repository.pending = null;
    repository.state = const HistorySpoilerState(1, {'clip'});
    first.complete(const HistorySpoilerState(0, {}));
    await Future.wait(refreshes);
    expect(repository.reads, 2);
    expect(controller.isRevealed('clip'), isTrue);
  });

  test(
    'failed reveal stays concealed and disposal ignores in-flight reads',
    () async {
      final repository = _Repository()..fail = true;
      final controller = HistorySpoilerController(repository);
      expect(await controller.setRevealed('clip', true), isFalse);
      expect(controller.isRevealed('clip'), isFalse);
      repository.pending = Completer<HistorySpoilerState>();
      final refresh = controller.refresh();
      controller.dispose();
      repository.pending!.complete(const HistorySpoilerState(10, {'clip'}));
      await refresh;
      expect(controller.isRevealed('clip'), isFalse);
    },
  );
}

class _Repository implements HistorySpoilerRepository {
  HistorySpoilerState state = const HistorySpoilerState(0, {});
  Completer<HistorySpoilerState>? pending;
  bool fail = false;
  int reads = 0;

  @override
  Future<HistorySpoilerState> readSpoilers() async {
    reads++;
    return pending == null ? state : await pending!.future;
  }

  @override
  Future<HistorySpoilerState> setSpoilerRevealed(
    String? id,
    bool revealed,
  ) async {
    if (fail) throw StateError('Unavailable');
    return state = HistorySpoilerState(state.revision + 1, {
      if (id != null) ...state.revealedIds.where((value) => value != id),
      if (id != null && revealed) id,
    });
  }
}
