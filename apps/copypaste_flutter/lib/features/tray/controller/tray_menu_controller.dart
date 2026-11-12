import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import '../../../platform/security/device_authenticator.dart';
import '../../../shared/runtime_event_service.dart';
import '../models/tray_menu_model.dart';
import '../repository/runtime_tray_repository.dart';
import '../repository/tray_clip_source.dart';
import '../repository/tray_preferences_store.dart';
import 'tray_recent_section.dart';

/// Feeds the tray and menu-bar menu with its feature rows and carries out
/// what they ask: copying Recent clips (secrets only after device
/// authentication), copying the latest clip from another device, capturing
/// the clipboard now, and handing New snippet, the paste queue and content
/// rules to the screens that own them.
class TrayMenuController implements TrayMenuDelegate {
  TrayMenuController({
    required TrayMenuSurface surface,
    TrayClipSource source = const RuntimeTrayClipSource(),
    DeviceAuthenticator? authenticator,
    TrayPreferencesStore? preferencesStore,
    bool? loadOnOpen,
    Stream<Object?>? historyChanges,
    DateTime Function()? clock,
  }) : _surface = surface,
       _source = source,
       _authenticator = authenticator ?? MethodChannelDeviceAuthenticator(),
       _store = preferencesStore ?? SharedPreferencesTrayPreferencesStore(),
       // A Linux D-Bus menu reports no opening, so it follows history instead.
       _loadOnOpen = loadOnOpen ?? !Platform.isLinux,
       _historyChanges = historyChanges,
       _clock = clock ?? DateTime.now {
    _recent = TrayRecentSection(source: source, authenticator: _authenticator);
  }

  /// How long one successful authentication unlocks secret copies.
  static const unlockWindow = Duration(seconds: 60);

  /// Coalesces a burst of history changes into one Recent reload.
  static const historyDebounce = Duration(seconds: 1);

  /// A closing menu delivers the chosen item after it reports closing, so
  /// previews are dropped only once that choice has arrived.
  static const releaseDelay = Duration(milliseconds: 500);

  final TrayMenuSurface _surface;
  final TrayClipSource _source;
  final DeviceAuthenticator _authenticator;
  final TrayPreferencesStore _store;
  final bool _loadOnOpen;
  final Stream<Object?>? _historyChanges;
  final DateTime Function() _clock;
  late final TrayRecentSection _recent;
  final ValueNotifier<TrayPreferences> _preferences = ValueNotifier(
    const TrayPreferences(),
  );
  StreamSubscription<Object?>? _historySubscription;
  Timer? _historyDebounce;
  Timer? _release;
  VoidCallback? _newSnippet;
  VoidCallback? _openPasteQueue;
  VoidCallback? _openContentRules;
  DateTime? _unlockedUntil;
  bool _open = false;
  bool _started = false;
  bool _disposed = false;

  ValueListenable<TrayPreferences> get preferences => _preferences;

  /// Loads preferences and starts receiving the menu's events.
  Future<void> start() async {
    if (_started || _disposed) return;
    _started = true;
    try {
      _preferences.value = await _store.read();
    } catch (_) {
      // The defaults apply when the stored choice cannot be read.
    }
    if (_disposed) return;
    _surface.setTrayMenuDelegate(this);
    await _update(
      (state) => state.copyWith(
        clipsAvailable: true,
        previewLevel: _preferences.value.previewLevel,
      ),
    );
    if (!_loadOnOpen) {
      _historySubscription = (_historyChanges ?? _defaultHistoryChanges())
          .listen(
            (_) => _scheduleReload(),
            onError: (Object _) {
              // The watch reconnects on its own and reports ready again.
            },
          );
      await _reload();
    }
  }

  Future<void> setPreferences(TrayPreferences preferences) async {
    if (_disposed || preferences == _preferences.value) return;
    _preferences.value = preferences;
    await _update(
      (state) => state.copyWith(previewLevel: preferences.previewLevel),
    );
    if (_open || !_loadOnOpen) await _reload();
    await _store.write(preferences);
  }

  /// The registered Quick Paste shortcut as the platform writes it.
  Future<void> setQuickPasteShortcut(String? shortcut) =>
      _update((state) => state.copyWith(quickPasteShortcut: shortcut));

  /// Opens the snippet editor; the item is disabled while none is set.
  Future<void> setNewSnippetHandler(VoidCallback? handler) {
    _newSnippet = handler;
    return _update(
      (state) => state.copyWith(newSnippetAvailable: handler != null),
    );
  }

  /// The paste queue slot: its size and whether it is collecting.
  Future<void> setPasteQueue(TrayQueueStatus? status) =>
      _update((state) => state.copyWith(queue: status));

  Future<void> setPasteQueueHandler(VoidCallback? handler) {
    _openPasteQueue = handler;
    return _update(
      (state) => state.copyWith(pasteQueueAvailable: handler != null),
    );
  }

  Future<void> setContentRulesHandler(VoidCallback? handler) {
    _openContentRules = handler;
    return _update(
      (state) => state.copyWith(contentRulesAvailable: handler != null),
    );
  }

  @override
  void trayMenuOpened() {
    _open = true;
    _release?.cancel();
    _release = null;
    if (_loadOnOpen) unawaited(_reload());
  }

  @override
  void trayMenuClosed() {
    _open = false;
    if (!_loadOnOpen || _disposed) return;
    _release?.cancel();
    _release = Timer(releaseDelay, () {
      _release = null;
      if (!_open) unawaited(_update(_recent.release));
    });
  }

  @override
  Future<void> trayActionSelected(TrayAction action) async {
    if (_disposed) return;
    try {
      switch (action) {
        case TrayCopyClipAction(:final clipId):
          await _copy(clipId, masked: _recent.requiresUnlock(clipId));
        case TrayCommandAction(command: TrayCommand.copyLatestFromOtherDevice):
          final latest = await _source.latestFromOtherDevice();
          if (latest != null) await _copy(latest.id, masked: latest.masked);
        case TrayCommandAction(command: TrayCommand.captureNow):
          await _source.captureClipboard();
        case TrayCommandAction(command: TrayCommand.newSnippet):
          await _reveal(_newSnippet);
        case TrayCommandAction(command: TrayCommand.openPasteQueue):
          await _reveal(_openPasteQueue);
        case TrayCommandAction(command: TrayCommand.openContentRules):
          await _reveal(_openContentRules);
        default:
          break;
      }
    } catch (_) {
      // A menu has nowhere to report a failed action; History shows state.
    }
  }

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    _historyDebounce?.cancel();
    _release?.cancel();
    await _historySubscription?.cancel();
    _surface.setTrayMenuDelegate(null);
    await _surface.updateTrayMenu(
      (state) => _recent.release(state).copyWith(clipsAvailable: false),
    );
    _preferences.dispose();
  }

  Future<void> _copy(String clipId, {required bool masked}) async {
    if (masked && !await _unlock()) return;
    await _source.copy(clipId);
  }

  /// A secret is copied only after the device owner authenticated, which
  /// then holds for [unlockWindow]. Without device authentication (Linux, or
  /// no screen lock set up) secrets cannot be copied from the menu.
  Future<bool> _unlock() async {
    final until = _unlockedUntil;
    if (until != null && _clock().isBefore(until)) return true;
    final result = await _authenticator.authenticate(
      reason: deviceAuthReason(
        AppStrings.current,
        DeviceAuthPurpose.copySecret,
      ),
    );
    if (result != DeviceAuthResult.success) return false;
    _unlockedUntil = _clock().add(unlockWindow);
    return true;
  }

  Future<void> _reveal(VoidCallback? handler) async {
    if (handler == null) return;
    await _surface.showFromTrayOrDock();
    if (!_disposed) handler();
  }

  void _scheduleReload() {
    _historyDebounce?.cancel();
    _historyDebounce = Timer(historyDebounce, () => unawaited(_reload()));
  }

  Future<void> _reload() async {
    if (_disposed) return;
    if (_surface.trayState.recentStatus != TrayLoadStatus.ready) {
      await _update(_recent.loading);
    }
    final apply = await _recent.load(_preferences.value.recentLimit);
    if (apply != null && !_disposed && (_open || !_loadOnOpen)) {
      await _update(apply);
    }
  }

  Future<void> _update(TrayMenuState Function(TrayMenuState) update) {
    if (_disposed) return Future<void>.value();
    return _surface.updateTrayMenu(update);
  }

  static Stream<Object?> _defaultHistoryChanges() =>
      RuntimeEventService.instance.eventsOf({RuntimeEventKind.items});
}
