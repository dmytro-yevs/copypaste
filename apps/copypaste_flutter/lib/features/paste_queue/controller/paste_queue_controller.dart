import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../platform/desktop/quick_paste_host.dart';
import '../../history/models/history_models.dart';
import '../../tray/models/tray_menu_model.dart';
import '../models/paste_queue_models.dart';
import '../repository/paste_queue_repository.dart';

/// Shows queue feedback outside the application window, for actions a global
/// shortcut ran while another application has focus.
abstract interface class PasteQueueNotifier {
  Future<void> show(PasteQueueFeedback feedback);
}

enum PasteQueueLoadStatus { loading, ready, failed }

/// A lasting reason Paste next only copies, shown until it is resolved.
enum PasteQueueBanner { accessibilityMissing, pasteUnsupported }

/// The device's paste queue: its state, its actions and ordered delivery.
///
/// Every delivery runs in one serialized chain, so rapid presses each
/// deliver exactly one entry in order, with a settle period between
/// clipboard writes. Entries are retained only while a view holds
/// [retainEntries]; otherwise the controller keeps the counts the tray and
/// header need.
class PasteQueueController extends ChangeNotifier {
  PasteQueueController({
    required PasteQueueRepository repository,
    required ForegroundPasteHost pasteHost,
    PasteQueueNotifier? notifier,
    Future<bool> Function()? requestAccessibility,
    Future<void> Function()? hideWindow,
    DateTime Function()? clock,
    this.settle = const Duration(milliseconds: 150),
    this.loadingDelay = const Duration(milliseconds: 150),
  }) : _repository = repository,
       _pasteHost = pasteHost,
       _notifier = notifier,
       _requestAccessibility = requestAccessibility,
       _hideWindow = hideWindow,
       _clock = clock ?? DateTime.now;

  /// The least time between two clipboard writes, so the target app reads
  /// one before the next replaces it.
  final Duration settle;

  /// Loads shorter than this show no loading state.
  final Duration loadingDelay;

  final PasteQueueRepository _repository;
  final ForegroundPasteHost _pasteHost;
  final PasteQueueNotifier? _notifier;
  final Future<bool> Function()? _requestAccessibility;
  final Future<void> Function()? _hideWindow;
  final DateTime Function() _clock;

  final _feedback = StreamController<PasteQueueFeedback>.broadcast();
  final _trayStatus = ValueNotifier<TrayQueueStatus?>(null);
  StreamSubscription<int?>? _changes;
  Future<void> _deliveries = Future<void>.value();
  Future<void>? _reloading;
  bool _reloadAgain = false;
  Timer? _loadingTimer;
  DateTime? _lastWrite;

  PasteQueueSnapshot _snapshot = PasteQueueSnapshot.initial;

  /// Entries of the last read, kept while a view or a delivery needs them.
  List<PasteQueueEntry> _lastEntries = const [];
  PasteQueueLoadStatus _status = PasteQueueLoadStatus.loading;
  bool _loadingVisible = false;
  int _pendingCount = 0;
  int _viewers = 0;
  int _queuedDeliveries = 0;
  PasteQueueBanner? _banner;
  PasteNextStyle _pasteNextStyle = PasteNextStyle.pasteIntoApp;
  String? _errorCode;
  bool _started = false;
  bool _disposed = false;

  PasteQueueSnapshot get snapshot => _snapshot;
  PasteQueueSettings get settings => _snapshot.settings;
  bool get enabled => _snapshot.settings.enabled;
  PasteQueueLoadStatus get status => _status;

  /// True only once a load has taken longer than [loadingDelay].
  bool get loadingVisible => _loadingVisible;
  int get pendingCount => _pendingCount;
  bool get collecting => _snapshot.collecting;
  PasteQueueBanner? get banner => _banner;

  /// The code of the last failed window action, cleared by the next one.
  String? get errorCode => _errorCode;

  /// Whether a delivery is running or waiting; the window must stay able to
  /// paste until it ends.
  bool get deliveryInFlight => _queuedDeliveries > 0;

  bool get undoAvailable {
    final until = _snapshot.undoUntil;
    return until != null && _clock().isBefore(until);
  }

  /// State changes to announce and, for shortcuts, to notify.
  Stream<PasteQueueFeedback> get feedback => _feedback.stream;

  /// The tray's queue slot; `null` while the feature is off.
  ValueListenable<TrayQueueStatus?> get trayStatus => _trayStatus;

  PasteNextStyle get pasteNextStyle => _pasteNextStyle;

  /// Chosen per device by the shortcut settings.
  set pasteNextStyle(PasteNextStyle style) {
    if (style == _pasteNextStyle) return;
    _pasteNextStyle = style;
    if (style == PasteNextStyle.clipboardOnly) _banner = null;
    _notify();
  }

  Future<void> start() async {
    if (_started || _disposed) return;
    _started = true;
    _changes = _repository.changes().listen(
      (revision) {
        if (revision == null || revision > _snapshot.revision) {
          unawaited(reload());
        }
      },
      onError: (Object _) {
        // The runtime watch reconnects by itself and then reports ready.
      },
    );
    await reload();
  }

  /// Keeps the entries in memory until the returned callback runs.
  VoidCallback retainEntries() {
    _viewers += 1;
    if (_viewers == 1 && _status == PasteQueueLoadStatus.ready) {
      unawaited(reload());
    }
    var released = false;
    return () {
      if (released) return;
      released = true;
      _viewers -= 1;
      if (_viewers == 0 && !_disposed) {
        _snapshot = _snapshot.withoutEntries();
        if (_queuedDeliveries == 0) _lastEntries = const [];
        _notify();
      }
    };
  }

  /// Reads the queue again; overlapping requests share one read and repeat
  /// it once if a change arrived meanwhile.
  Future<void> reload() {
    if (_disposed) return Future.value();
    final running = _reloading;
    if (running != null) {
      _reloadAgain = true;
      return running;
    }
    final load = _reloading = _load();
    return load.whenComplete(() => _reloading = null);
  }

  Future<void> _load() async {
    do {
      _reloadAgain = false;
      if (_status != PasteQueueLoadStatus.ready) {
        _loadingTimer ??= Timer(loadingDelay, () {
          _loadingVisible = true;
          _notify();
        });
      }
      try {
        _apply(await _repository.state());
      } on Object {
        if (_status != PasteQueueLoadStatus.ready) {
          _status = PasteQueueLoadStatus.failed;
        }
      } finally {
        _loadingTimer?.cancel();
        _loadingTimer = null;
        _loadingVisible = false;
        _notify();
      }
    } while (_reloadAgain && !_disposed);
  }

  /// Turns collecting on or off. [append] keeps the entries of a finished
  /// queue; by default they are cleared for a fresh run.
  Future<bool> setCollecting(
    bool on, {
    bool append = false,
    PasteQueueTrigger trigger = PasteQueueTrigger.window,
  }) async {
    if (!enabled) return false;
    final done = await _action(
      () => _repository.setCollecting(on, append: append),
    );
    if (done) {
      _emit(
        PasteQueueFeedback(
          on
              ? PasteQueueFeedbackKind.collectingStarted
              : PasteQueueFeedbackKind.collectingStopped,
          trigger: trigger,
          total: _pendingCount,
        ),
      );
    }
    return done;
  }

  Future<bool> toggleCollecting({
    PasteQueueTrigger trigger = PasteQueueTrigger.window,
  }) => setCollecting(!collecting, trigger: trigger);

  Future<bool> add(List<PasteQueueSource> sources) async {
    if (!enabled || sources.isEmpty) return false;
    final done = await _action(() => _repository.add(sources));
    if (done) {
      _emit(
        PasteQueueFeedback(
          PasteQueueFeedbackKind.added,
          trigger: PasteQueueTrigger.window,
          count: sources.length,
          total: _pendingCount,
        ),
      );
    }
    return done;
  }

  Future<bool> remove(String entryId) =>
      _action(() => _repository.remove([entryId]));

  Future<bool> removeFinished() => _action(_repository.removeFinished);

  Future<bool> clear() => _action(_repository.clear);

  Future<bool> undoClear() => _action(_repository.undoClear);

  Future<bool> reverse() => _action(_repository.reverse);

  Future<bool> move(String entryId, int toIndex) =>
      _action(() => _repository.move(entryId, toIndex));

  /// Sets the entry's queue-local text; history keeps the original.
  Future<bool> edit(String entryId, String? text) =>
      _action(() => _repository.edit(entryId, text));

  Future<bool> stepBack({
    PasteQueueTrigger trigger = PasteQueueTrigger.window,
  }) => _serialized(() => _action(_repository.stepBack));

  Future<bool> skip({PasteQueueTrigger trigger = PasteQueueTrigger.window}) =>
      _serialized(() => _action(_repository.skip));

  Future<bool> updateSettings(PasteQueueSettings settings) async {
    if (_disposed) return false;
    _errorCode = null;
    try {
      final applied = await _repository.setSettings(settings);
      if (_disposed) return false;
      _snapshot = _withSettings(_snapshot, applied);
      _updateTray();
      _notify();
      unawaited(reload());
      return true;
    } on PasteQueueException catch (error) {
      _errorCode = error.code;
      _notify();
      return false;
    }
  }

  /// Delivers the next pending entry. Presses queue behind each other and
  /// are never merged.
  Future<void> pasteNext({
    bool plainText = false,
    PasteQueueTrigger trigger = PasteQueueTrigger.window,
  }) => _serialized(
    () =>
        _deliver(PasteQueueTarget.next, plainText: plainText, trigger: trigger),
  );

  /// Delivers every pending entry as one paste, joined by the separator.
  Future<void> pasteAll({
    PasteQueueTrigger trigger = PasteQueueTrigger.window,
  }) => _serialized(() => _deliver(PasteQueueTarget.all, trigger: trigger));

  /// Delivers [entryId] now, out of order.
  Future<void> pasteEntry(String entryId) => _serialized(
    () => _deliver(
      PasteQueueTarget.entry,
      entryId: entryId,
      trigger: PasteQueueTrigger.window,
    ),
  );

  /// Puts every pending entry, joined, on the clipboard without pasting.
  Future<void> copyAll() => _serialized(
    () => _deliver(
      PasteQueueTarget.all,
      trigger: PasteQueueTrigger.window,
      copyOnly: true,
    ),
  );

  /// Asks for the Accessibility permission Paste next needs.
  Future<void> requestAccessibility() async {
    final request = _requestAccessibility;
    if (request == null) return;
    try {
      if (await request()) dismissBanner();
    } on Object {
      // The banner stays; the permission can still be granted in Settings.
    }
  }

  void dismissBanner() {
    if (_banner == null) return;
    _banner = null;
    _notify();
  }

  Future<T> _serialized<T>(Future<T> Function() job) {
    _queuedDeliveries += 1;
    final run = _deliveries.then((_) => job());
    _deliveries = run.then<void>(
      (_) {},
      onError: (Object error, StackTrace stackTrace) {},
    );
    return run.whenComplete(() {
      _queuedDeliveries -= 1;
      if (_queuedDeliveries > 0) return;
      if (_viewers == 0) _lastEntries = const [];
      _notify();
    });
  }

  Future<void> _deliver(
    PasteQueueTarget target, {
    required PasteQueueTrigger trigger,
    String? entryId,
    bool plainText = false,
    bool copyOnly = false,
  }) async {
    if (_disposed || !enabled) return;
    final viaWindow = trigger == PasteQueueTrigger.window;
    final hideWindow = _hideWindow;
    final paste =
        !copyOnly &&
        _pasteNextStyle == PasteNextStyle.pasteIntoApp &&
        (!viaWindow || hideWindow != null);
    if (paste && viaWindow) await hideWindow!();
    await _waitForSettle(force: paste && viaWindow);
    final PasteQueueDelivery delivery;
    try {
      delivery = await _repository.deliver(
        target,
        entryId: entryId,
        plainText: plainText ? true : null,
        mode: paste
            ? PasteQueueDeliveryMode.clipboard
            : PasteQueueDeliveryMode.copyOnly,
      );
    } on PasteQueueException catch (error) {
      _errorCode = error.code;
      _emit(
        PasteQueueFeedback(PasteQueueFeedbackKind.failed, trigger: trigger),
      );
      return;
    }
    if (delivery.unavailable > 0) {
      _emit(
        PasteQueueFeedback(
          PasteQueueFeedbackKind.unavailable,
          trigger: trigger,
          count: delivery.unavailable,
        ),
      );
    }
    final nothing = delivery.nothing;
    if (nothing != null || delivery.entryIds.isEmpty) {
      await reload();
      _emit(
        PasteQueueFeedback(
          nothing == PasteQueueNothing.mixed
              ? PasteQueueFeedbackKind.mixed
              : PasteQueueFeedbackKind.nothingToPaste,
          trigger: trigger,
        ),
      );
      return;
    }
    _lastWrite = _clock();
    final token = delivery.token;
    var kind = PasteQueueFeedbackKind.copied;
    var inserted = true;
    if (paste && token != null) {
      PasteOutcome outcome;
      try {
        outcome = await _pasteHost.pasteForeground(plainText: plainText);
      } on Object {
        outcome = PasteOutcome.unsupported;
      }
      switch (outcome) {
        case PasteOutcome.pasted:
          kind = PasteQueueFeedbackKind.pasted;
          _banner = null;
        case PasteOutcome.copiedOnly:
          final granted = await _accessibilityGranted();
          kind = granted
              ? PasteQueueFeedbackKind.pasteUnsupported
              : PasteQueueFeedbackKind.accessibilityMissing;
          if (!granted) _banner = PasteQueueBanner.accessibilityMissing;
        case PasteOutcome.unsupported:
          kind = PasteQueueFeedbackKind.pasteUnsupported;
          _banner = PasteQueueBanner.pasteUnsupported;
        case PasteOutcome.elevatedTarget:
          kind = PasteQueueFeedbackKind.targetElevated;
          inserted = false;
        case PasteOutcome.noTarget:
          kind = PasteQueueFeedbackKind.noTarget;
          inserted = false;
      }
    }
    if (token != null) {
      // The clipboard-only fallbacks still consume the entry: it is on the
      // clipboard, and keeping it would deliver the same entry again.
      if (!await _action(
        () => _repository.confirm(token, inserted: inserted),
      )) {
        await reload();
      }
    } else {
      await reload();
    }
    _emit(
      PasteQueueFeedback(
        kind,
        trigger: trigger,
        position: _positionOf(delivery.entryIds.last),
        total: _deliverableTotal,
        count: delivery.entryIds.length,
      ),
    );
    if (inserted && delivery.finished) {
      _emit(
        PasteQueueFeedback(PasteQueueFeedbackKind.finished, trigger: trigger),
      );
    }
  }

  Future<bool> _accessibilityGranted() async {
    try {
      return await _pasteHost.accessibilityGranted();
    } on Object {
      return false;
    }
  }

  Future<void> _waitForSettle({required bool force}) async {
    final last = _lastWrite;
    final elapsed = last == null ? settle : _clock().difference(last);
    // A window that just hid needs the whole period to hand focus back.
    final wait = force ? settle : settle - elapsed;
    if (wait > Duration.zero) await Future<void>.delayed(wait);
  }

  int get _deliverableTotal => _lastEntries
      .where((entry) => entry.status != PasteQueueEntryStatus.unavailable)
      .length;

  int _positionOf(String entryId) {
    var position = 0;
    for (final entry in _lastEntries) {
      if (entry.status == PasteQueueEntryStatus.unavailable) continue;
      position += 1;
      if (entry.id == entryId) return position;
    }
    return 0;
  }

  Future<bool> _action(Future<PasteQueueSnapshot> Function() request) async {
    if (_disposed) return false;
    _errorCode = null;
    try {
      _apply(await request());
      return true;
    } on PasteQueueException catch (error) {
      if (_disposed) return false;
      _errorCode = error.code;
      _notify();
      return false;
    }
  }

  void _apply(PasteQueueSnapshot next) {
    if (_disposed || next.revision < _snapshot.revision) return;
    _status = PasteQueueLoadStatus.ready;
    _pendingCount = next.pendingCount;
    final retained = _viewers > 0;
    // A delivery reads positions from the entries after it lands.
    _lastEntries = retained || _queuedDeliveries > 0 ? next.entries : const [];
    _snapshot = retained ? next : next.withoutEntries();
    _updateTray();
    _notify();
  }

  void _updateTray() {
    _trayStatus.value = enabled
        ? TrayQueueStatus(count: _pendingCount, collecting: collecting)
        : null;
  }

  void _emit(PasteQueueFeedback feedback) {
    if (_disposed) return;
    _feedback.add(feedback);
    final notifier = _notifier;
    if (notifier != null &&
        feedback.trigger == PasteQueueTrigger.shortcut &&
        feedback.kind != PasteQueueFeedbackKind.pasted &&
        feedback.kind != PasteQueueFeedbackKind.copied) {
      unawaited(
        notifier.show(feedback).catchError((Object _) {
          // Notifications are best effort; the tray still shows the queue.
        }),
      );
    }
    _notify();
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  /// Loads [HistorySourceAppIcon]s for rows; views cache what they show.
  Future<HistorySourceAppIcon?> sourceAppIcon(String? iconId) async {
    if (iconId == null || _disposed) return null;
    try {
      return await _repository.sourceAppIcon(iconId);
    } on Object {
      return null;
    }
  }

  static PasteQueueSnapshot _withSettings(
    PasteQueueSnapshot snapshot,
    PasteQueueSettings settings,
  ) => PasteQueueSnapshot(
    revision: snapshot.revision,
    collecting: snapshot.collecting,
    phase: snapshot.phase,
    full: snapshot.full,
    entries: snapshot.entries,
    staleSince: snapshot.staleSince,
    undoUntil: snapshot.undoUntil,
    settings: settings,
  );

  @override
  void dispose() {
    _disposed = true;
    _loadingTimer?.cancel();
    unawaited(_changes?.cancel());
    unawaited(_feedback.close());
    _trayStatus.dispose();
    super.dispose();
  }
}
