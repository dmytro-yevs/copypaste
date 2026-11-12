import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../../../l10n/l10n.dart';
import '../../../platform/desktop/global_shortcut.dart';
import '../models/paste_queue_models.dart';
import '../presentation/paste_queue_messages.dart';
import '../repository/paste_queue_shortcuts_store.dart';
import 'paste_queue_controller.dart';

/// Why a queue shortcut is not active.
enum PasteQueueShortcutProblem {
  /// Another CopyPaste shortcut already uses the combination.
  heldByApp,

  /// Another application holds the combination.
  inUse,

  /// The system refused the combination.
  rejected,

  /// Ctrl+Alt is AltGr on many layouts and would block typing characters.
  altGr,

  /// This session offers no global shortcuts.
  unavailable,
  failed,
}

/// The paste queue's global shortcuts and delivery style on this device.
///
/// Shortcuts register only while the queue is enabled. A binding the system
/// or another shortcut refuses keeps the previous binding and reports the
/// refusal on its row.
class PasteQueueShortcutsController extends ChangeNotifier {
  PasteQueueShortcutsController({
    required PasteQueueController queue,
    required DesktopShortcutRegistrar registrar,
    PasteQueueShortcutsStore? store,
    TargetPlatform? platform,
  }) : _queue = queue,
       _registrar = registrar,
       _store = store ?? SharedPreferencesPasteQueueShortcutsStore(),
       _platform = platform ?? defaultTargetPlatform;

  final PasteQueueController _queue;
  final DesktopShortcutRegistrar _registrar;
  final PasteQueueShortcutsStore _store;
  final TargetPlatform _platform;

  PasteQueueDevicePreferences _preferences =
      const PasteQueueDevicePreferences();
  final Map<PasteQueueShortcutAction, DesktopShortcut> _registered = {};
  final Map<PasteQueueShortcutAction, PasteQueueShortcutProblem> _problems = {};
  final Map<PasteQueueShortcutAction, String> _heldBy = {};
  PasteQueueShortcutAction? _recording;
  Future<void> _registration = Future<void>.value();
  bool _busy = false;
  bool _started = false;
  bool _disposed = false;
  bool _enabled = false;

  bool get busy => _busy;
  PasteNextStyle get pasteNextStyle => _preferences.pasteNextStyle;

  /// Ctrl+Alt combinations are refused where Alt Graph types characters.
  bool get refusesControlAlt =>
      _platform == TargetPlatform.windows || _platform == TargetPlatform.linux;

  DesktopShortcut? shortcutFor(PasteQueueShortcutAction action) =>
      _preferences.shortcutFor(action, _platform);

  /// The binding active now, or `null` while unbound, off or refused.
  DesktopShortcut? registeredShortcut(PasteQueueShortcutAction action) =>
      _registered[action];

  PasteQueueShortcutProblem? problemOf(PasteQueueShortcutAction action) =>
      _problems[action];

  /// The registrar id that holds the combination [action] asked for.
  String? heldByOf(PasteQueueShortcutAction action) => _heldBy[action];

  /// Whether to prompt for shortcuts: collecting and Paste next both have
  /// none, as on a fresh Windows or Linux install.
  bool get needsShortcuts =>
      _queue.enabled &&
      shortcutFor(PasteQueueShortcutAction.toggleCollecting) == null &&
      shortcutFor(PasteQueueShortcutAction.pasteNext) == null;

  /// The queue drawer shows the prompt until it is dismissed once.
  bool get showsShortcutPrompt =>
      needsShortcuts && !_preferences.shortcutPromptDismissed;

  Future<void> start() async {
    if (_started || _disposed) return;
    _started = true;
    try {
      _preferences = await _store.read();
    } catch (_) {
      // Defaults apply when the stored choices cannot be read.
    }
    if (_disposed) return;
    _queue.pasteNextStyle = _preferences.pasteNextStyle;
    _queue.addListener(_queueChanged);
    _enabled = _queue.enabled && _queue.status == PasteQueueLoadStatus.ready;
    await _sync();
    _notify();
  }

  /// Whether [shortcut] may be recorded on this platform.
  bool accepts(DesktopShortcut shortcut) =>
      !refusesControlAlt ||
      !(shortcut.modifiers.contains(DesktopShortcutModifier.control) &&
          shortcut.modifiers.contains(DesktopShortcutModifier.alt));

  void reportRejected(PasteQueueShortcutAction action) {
    _problems[action] = PasteQueueShortcutProblem.altGr;
    _notify();
  }

  /// Releases [action]'s binding so the recorder receives its keys.
  Future<bool> beginRecording(PasteQueueShortcutAction action) async {
    if (_disposed || _busy || _recording != null) return false;
    _recording = action;
    await _sync();
    return !_disposed;
  }

  Future<void> cancelRecording(PasteQueueShortcutAction action) async {
    if (_recording != action) return;
    _recording = null;
    await _sync();
  }

  /// Binds [shortcut] to [action]; `null` unbinds it. A refused binding
  /// keeps the previous one and returns false.
  Future<bool> setShortcut(
    PasteQueueShortcutAction action,
    DesktopShortcut? shortcut,
  ) async {
    if (_disposed || _busy) return false;
    if (shortcut != null && (!shortcut.isValid || !accepts(shortcut))) {
      return false;
    }
    if (_recording == action) _recording = null;
    final before = _preferences;
    if (before.shortcutFor(action, _platform) == shortcut &&
        _registered[action] == shortcut) {
      _problems.remove(action);
      _notify();
      return false;
    }
    _busy = true;
    _notify();
    try {
      _preferences = before.withShortcut(action, shortcut);
      final applied = await _apply(action);
      if (!applied) {
        _preferences = before;
        await _sync();
        return false;
      }
      await _store.write(_preferences);
      return true;
    } catch (_) {
      _preferences = before;
      _problems[action] = PasteQueueShortcutProblem.failed;
      return false;
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<bool> resetShortcut(PasteQueueShortcutAction action) =>
      setShortcut(action, action.defaultFor(_platform));

  Future<void> setPasteNextStyle(PasteNextStyle style) async {
    if (_disposed || style == _preferences.pasteNextStyle) return;
    _preferences = _preferences.copyWith(pasteNextStyle: style);
    _queue.pasteNextStyle = style;
    _notify();
    await _persist();
  }

  Future<void> dismissShortcutPrompt() async {
    if (_disposed || _preferences.shortcutPromptDismissed) return;
    _preferences = _preferences.copyWith(shortcutPromptDismissed: true);
    _notify();
    await _persist();
  }

  Future<void> _persist() async {
    try {
      await _store.write(_preferences);
    } catch (_) {
      // The choice holds for this session; the next start reads the last
      // stored one.
    }
  }

  void _queueChanged() {
    final enabled =
        _queue.enabled && _queue.status == PasteQueueLoadStatus.ready;
    if (enabled == _enabled) return;
    _enabled = enabled;
    unawaited(_sync().then((_) => _notify()));
  }

  /// Registers what [action] should hold now and reports whether the
  /// registration took; the previous binding is kept when it did not.
  Future<bool> _apply(PasteQueueShortcutAction action) => _serialize(() async {
    final desired = _desired(action);
    final previous = _registered[action];
    if (desired == previous) return true;
    try {
      await _change(action, desired);
      _clearProblem(action);
      return true;
    } catch (error) {
      _recordProblem(action, error);
      if (previous != null && _registered[action] == null) {
        try {
          await _change(action, previous);
        } catch (_) {
          // The row reports the refusal; nothing else can be claimed.
        }
      }
      return false;
    }
  });

  /// Brings every registration in line with the enabled state and choices.
  Future<void> _sync() => _serialize(() async {
    for (final action in PasteQueueShortcutAction.values) {
      if (_disposed) return;
      final desired = _desired(action);
      if (desired == _registered[action]) continue;
      try {
        await _change(action, desired);
        if (_recording != action) _clearProblem(action);
      } catch (error) {
        _recordProblem(action, error);
      }
    }
  });

  DesktopShortcut? _desired(PasteQueueShortcutAction action) =>
      !_disposed && _enabled && _recording != action
      ? shortcutFor(action)
      : null;

  Future<void> _change(
    PasteQueueShortcutAction action,
    DesktopShortcut? shortcut,
  ) async {
    if (shortcut == null) {
      _registered.remove(action);
      await _registrar.unregister(action.id);
      return;
    }
    _registered.remove(action);
    await _registrar.register(
      action.id,
      shortcut,
      () => _run(action),
      description: _description(action),
    );
    _registered[action] = shortcut;
  }

  void _clearProblem(PasteQueueShortcutAction action) {
    _problems.remove(action);
    _heldBy.remove(action);
  }

  Future<T> _serialize<T>(Future<T> Function() operation) {
    final run = _registration.then((_) => operation());
    _registration = run.then<void>((_) {}, onError: (Object _) {});
    return run;
  }

  void _recordProblem(PasteQueueShortcutAction action, Object error) {
    if (error is DesktopShortcutConflict) {
      _problems[action] = PasteQueueShortcutProblem.heldByApp;
      _heldBy[action] = error.heldBy;
      return;
    }
    _problems[action] = switch (DesktopShortcutRefusal.reasonOf(error)) {
      DesktopShortcutRefusalReason.inUse => PasteQueueShortcutProblem.inUse,
      DesktopShortcutRefusalReason.rejected =>
        PasteQueueShortcutProblem.rejected,
      null
          when error is PlatformException &&
              error.code == 'shortcut_unavailable' =>
        PasteQueueShortcutProblem.unavailable,
      null => PasteQueueShortcutProblem.failed,
    };
    debugPrint('Paste Queue shortcut ${action.name} was refused: $error');
  }

  Future<void> _run(PasteQueueShortcutAction action) {
    const trigger = PasteQueueTrigger.shortcut;
    return switch (action) {
      PasteQueueShortcutAction.toggleCollecting => _queue.toggleCollecting(
        trigger: trigger,
      ),
      PasteQueueShortcutAction.pasteNext => _queue.pasteNext(trigger: trigger),
      PasteQueueShortcutAction.pasteNextPlain => _queue.pasteNext(
        plainText: true,
        trigger: trigger,
      ),
      PasteQueueShortcutAction.pasteAll => _queue.pasteAll(trigger: trigger),
      PasteQueueShortcutAction.stepBack => _queue.stepBack(trigger: trigger),
      PasteQueueShortcutAction.skip => _queue.skip(trigger: trigger),
    };
  }

  static String _description(PasteQueueShortcutAction action) =>
      AppStrings.current.pasteQueueShortcutAction(action);

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _queue.removeListener(_queueChanged);
    final registered = _registered.keys.toList();
    _registered.clear();
    unawaited(
      _registration.then((_) async {
        for (final action in registered) {
          try {
            await _registrar.unregister(action.id);
          } catch (_) {
            // Teardown cannot report late native registration errors.
          }
        }
      }),
    );
    super.dispose();
  }
}
