import '../../history/models/history_models.dart';
import '../models/paste_queue_models.dart';

/// Runtime access to this device's paste queue.
///
/// Failures are [PasteQueueException]s carrying the runtime error code.
abstract interface class PasteQueueRepository {
  Future<PasteQueueSnapshot> state();

  /// [append] keeps finished entries when collecting restarts after the
  /// queue finished; otherwise they are removed.
  Future<PasteQueueSnapshot> setCollecting(bool on, {bool append = false});

  Future<PasteQueueSnapshot> add(List<PasteQueueSource> sources);

  Future<PasteQueueSnapshot> remove(List<String> entryIds);

  /// Removes pasted, skipped and unavailable entries.
  Future<PasteQueueSnapshot> removeFinished();

  Future<PasteQueueSnapshot> clear();

  Future<PasteQueueSnapshot> undoClear();

  /// Reverses the pending entries.
  Future<PasteQueueSnapshot> reverse();

  Future<PasteQueueSnapshot> move(String entryId, int toIndex);

  /// Sets the entry's queue-local text; `null` drops the override.
  Future<PasteQueueSnapshot> edit(String entryId, String? text);

  /// Puts the target on the clipboard and marks it pasted. [entryId] is
  /// required for [PasteQueueTarget.entry].
  Future<PasteQueueDelivery> deliver(
    PasteQueueTarget target, {
    String? entryId,
    bool? plainText,
    required PasteQueueDeliveryMode mode,
  });

  /// Reports whether a [PasteQueueDeliveryMode.clipboard] delivery reached
  /// the target; a rejected one returns its entries to pending.
  Future<PasteQueueSnapshot> confirm(String token, {required bool inserted});

  Future<PasteQueueSnapshot> stepBack();

  Future<PasteQueueSnapshot> skip();

  Future<PasteQueueSettings> setSettings(PasteQueueSettings settings);

  Future<HistorySourceAppIcon?> sourceAppIcon(String iconId);

  /// Emits the new revision on every queue change, and `null` when the
  /// runtime connection (re)starts and everything must be read again.
  Stream<int?> changes();
}
