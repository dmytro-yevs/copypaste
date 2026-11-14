import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/generated/api/paste_queue.dart' as api;
import 'package:copypaste_flutter/generated/api/snippets.dart' as snippets;

import '../../../shared/runtime_event_service.dart';
import '../../history/models/history_models.dart';
import '../models/paste_queue_models.dart';
import 'paste_queue_repository.dart';

/// [PasteQueueRepository] over the runtime bridge.
class RuntimePasteQueueRepository implements PasteQueueRepository {
  RuntimePasteQueueRepository({RuntimeEventService? events})
    : _events = events ?? RuntimeEventService.instance;

  final RuntimeEventService _events;

  @override
  Future<PasteQueueSnapshot> state() => _view(api.pasteQueueState);

  @override
  Future<PasteQueueSnapshot> setCollecting(bool on, {bool append = false}) =>
      _view(() => api.setPasteQueueCollecting(on_: on, append: append));

  @override
  Future<PasteQueueSnapshot> add(List<PasteQueueSource> sources) => _view(
    () => api.addToPasteQueue(
      sources: [for (final source in sources) _source(source)],
    ),
  );

  @override
  Future<PasteQueueSnapshot> remove(List<String> entryIds) =>
      _view(() => api.removePasteQueueEntries(entryIds: entryIds));

  @override
  Future<PasteQueueSnapshot> removeFinished() =>
      _view(api.removePastedQueueEntries);

  @override
  Future<PasteQueueSnapshot> clear() => _view(api.clearPasteQueue);

  @override
  Future<PasteQueueSnapshot> undoClear() => _view(api.undoClearPasteQueue);

  @override
  Future<PasteQueueSnapshot> reverse() => _view(api.reversePasteQueue);

  @override
  Future<PasteQueueSnapshot> move(String entryId, int toIndex) =>
      _view(() => api.movePasteQueueEntry(entryId: entryId, toIndex: toIndex));

  @override
  Future<PasteQueueSnapshot> edit(String entryId, String? text) =>
      _view(() => api.editPasteQueueEntry(entryId: entryId, text: text));

  @override
  Future<PasteQueueDelivery> deliver(
    PasteQueueTarget target, {
    String? entryId,
    bool? plainText,
    required PasteQueueDeliveryMode mode,
  }) => _guard(() async {
    final delivery = await api.deliverFromPasteQueue(
      target: switch (target) {
        PasteQueueTarget.next => api.PasteQueueTargetKind.next,
        PasteQueueTarget.all => api.PasteQueueTargetKind.all,
        PasteQueueTarget.entry => api.PasteQueueTargetKind.entry,
      },
      entryId: entryId,
      plainText: plainText,
      mode: switch (mode) {
        PasteQueueDeliveryMode.clipboard =>
          api.PasteQueueDeliveryMode.clipboard,
        PasteQueueDeliveryMode.copyOnly => api.PasteQueueDeliveryMode.copyOnly,
      },
    );
    return PasteQueueDelivery(
      token: delivery.token,
      entryIds: delivery.entryIds,
      unavailable: delivery.unavailable,
      finished: delivery.finished,
      nothing: switch (delivery.nothingToPaste) {
        api.PasteQueueNothingToPaste.empty => PasteQueueNothing.empty,
        api.PasteQueueNothingToPaste.finished => PasteQueueNothing.finished,
        api.PasteQueueNothingToPaste.mixed => PasteQueueNothing.mixed,
        null => null,
      },
    );
  });

  @override
  Future<PasteQueueSnapshot> confirm(String token, {required bool inserted}) =>
      _view(
        () => api.confirmPasteQueueDelivery(token: token, inserted: inserted),
      );

  @override
  Future<PasteQueueSnapshot> stepBack() => _view(api.stepBackPasteQueue);

  @override
  Future<PasteQueueSnapshot> skip() => _view(api.skipPasteQueueEntry);

  @override
  Future<PasteQueueSettings> setSettings(PasteQueueSettings settings) => _guard(
    () async => _settings(
      await api.setPasteQueuePreferences(value: _preferences(settings)),
    ),
  );

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String iconId) =>
      _guard(() async {
        final icon = await runtime.sourceAppIcon(id: iconId);
        if (icon == null) return null;
        return HistorySourceAppIcon(
          icon.pngBytes,
          width: icon.width,
          height: icon.height,
        );
      });

  @override
  Stream<int?> changes() => _events
      .eventsOf(const {RuntimeEventKind.pasteQueue})
      .map(
        (event) => event.kind == RuntimeEventKind.ready
            ? null
            : event.pasteQueueRevision?.toInt(),
      );

  static Future<PasteQueueSnapshot> _view(
    Future<api.PasteQueueView> Function() request,
  ) => _guard(() async => _snapshot(await request()));

  static Future<T> _guard<T>(Future<T> Function() request) async {
    try {
      return await request();
    } on runtime.RuntimeError catch (error) {
      throw PasteQueueException(error.code, error.message);
    }
  }

  static api.PasteQueueSourceInput _source(PasteQueueSource source) =>
      switch (source) {
        PasteQueueClipSource(:final id) => api.PasteQueueSourceInput(
          kind: api.PasteQueueSourceKind.clip,
          id: id,
          values: const [],
        ),
        PasteQueueSnippetSource(:final id, :final values) =>
          api.PasteQueueSourceInput(
            kind: api.PasteQueueSourceKind.snippet,
            id: id,
            values: [
              for (final MapEntry(:key, :value) in values.entries)
                snippets.TemplateValue(name: key, value: value),
            ],
          ),
      };

  static PasteQueueSnapshot _snapshot(api.PasteQueueView view) =>
      PasteQueueSnapshot(
        revision: view.revision.toInt(),
        collecting: view.collecting,
        phase: switch (view.phase) {
          api.PasteQueuePhase.empty => PasteQueuePhase.empty,
          api.PasteQueuePhase.active => PasteQueuePhase.active,
          api.PasteQueuePhase.finished => PasteQueuePhase.finished,
        },
        full: view.full,
        entries: [for (final entry in view.entries) _entry(entry)],
        staleSince: _time(view.staleSinceMs),
        undoUntil: _time(view.undoUntilMs),
        settings: _settings(view.settings),
      );

  static PasteQueueEntry _entry(api.PasteQueueEntryView entry) =>
      PasteQueueEntry(
        id: entry.entryId,
        position: entry.positionLabel,
        status: switch (entry.status) {
          api.PasteQueueEntryStatus.pending => PasteQueueEntryStatus.pending,
          api.PasteQueueEntryStatus.pasted => PasteQueueEntryStatus.pasted,
          api.PasteQueueEntryStatus.skipped => PasteQueueEntryStatus.skipped,
          api.PasteQueueEntryStatus.unavailable =>
            PasteQueueEntryStatus.unavailable,
        },
        kind: switch (entry.contentKind) {
          api.PasteQueueContentKind.text => PasteQueueContentKind.text,
          api.PasteQueueContentKind.richText => PasteQueueContentKind.richText,
          api.PasteQueueContentKind.image => PasteQueueContentKind.image,
          api.PasteQueueContentKind.files => PasteQueueContentKind.files,
          api.PasteQueueContentKind.link => PasteQueueContentKind.link,
        },
        preview: entry.preview,
        masked: entry.masked,
        edited: entry.edited,
        sourceAppName: entry.sourceAppName,
        sourceAppIconId: entry.sourceAppIconId,
        renderedAt: _time(entry.renderedAtMs),
      );

  static DateTime? _time(int? ms) =>
      ms == null ? null : DateTime.fromMillisecondsSinceEpoch(ms);

  static PasteQueueSettings _settings(api.PasteQueuePreferences value) =>
      PasteQueueSettings(
        enabled: value.enabled,
        insertAtStart: value.insertAtStart,
        endBehavior: switch (value.endBehavior) {
          api.PasteQueueEndBehavior.stop => PasteQueueEndBehavior.stop,
          api.PasteQueueEndBehavior.loop => PasteQueueEndBehavior.loop,
          api.PasteQueueEndBehavior.keepLast => PasteQueueEndBehavior.keepLast,
        },
        separator: PasteQueueSeparator.values.byName(value.separator.name),
        customSeparator: value.customSeparator,
        plainText: value.plainText,
        idleStopMinutes: value.idleStopMinutes,
        maxEntries: value.maxEntries,
        moveToNextField: value.moveToNextField,
        restoreWhenFinished: value.restoreWhenFinished,
      );

  static api.PasteQueuePreferences _preferences(PasteQueueSettings value) =>
      api.PasteQueuePreferences(
        enabled: value.enabled,
        insertAtStart: value.insertAtStart,
        endBehavior: switch (value.endBehavior) {
          PasteQueueEndBehavior.stop => api.PasteQueueEndBehavior.stop,
          PasteQueueEndBehavior.loop => api.PasteQueueEndBehavior.loop,
          PasteQueueEndBehavior.keepLast => api.PasteQueueEndBehavior.keepLast,
        },
        separator: api.PasteQueueSeparator.values.byName(value.separator.name),
        customSeparator: value.customSeparator,
        plainText: value.plainText,
        idleStopMinutes: value.idleStopMinutes,
        maxEntries: value.maxEntries,
        moveToNextField: value.moveToNextField,
        restoreWhenFinished: value.restoreWhenFinished,
      );
}
