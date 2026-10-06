import 'dart:async';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../../platform/desktop/quick_paste_host.dart';
import '../history/controller/history_controller.dart';
import '../history/models/history_models.dart';
import '../history/repository/history_repository.dart';
import '../settings/repository/quick_paste_preferences_store.dart';

class QuickPasteController extends ChangeNotifier {
  QuickPasteController({
    required HistoryRepository repository,
    Future<void> Function()? disposeRepository,
    required QuickPastePreferencesStore preferencesStore,
    required QuickPasteContextHost host,
  }) : _disposeRepository = disposeRepository,
       _preferencesStore = preferencesStore,
       _host = host,
       history = HistoryController(
         repository,
         pageSize: 50,
         searchDebounce: Duration.zero,
       ),
       _preferences = QuickPastePreferences.defaults() {
    history.addListener(_historyChanged);
    _host.setOpenedHandler(opened);
    _host.setShutdownHandler(shutdown);
  }

  final Future<void> Function()? _disposeRepository;
  final QuickPastePreferencesStore _preferencesStore;
  final QuickPasteContextHost _host;
  final HistoryController history;

  QuickPastePreferences _preferences;
  HistoryClip? _focusedClip;
  bool _accessibilityGranted = false;
  bool _initialized = false;
  bool _activating = false;
  int _presentationGeneration = 0;
  int? _presentationId;
  bool _disposed = false;
  bool _inspectorOpen = false;
  Map<String, String> _pinnedShortcuts = {};
  Future<void> _shortcutSync = Future<void>.value();
  Future<void>? _repositoryDisposal;

  bool get autoPaste => _preferences.autoPaste;
  bool get accessibilityGranted => _accessibilityGranted;
  bool get activating => _activating;
  int get presentationGeneration => _presentationGeneration;
  HistoryClip? get focusedClip => _focusedClip;
  bool get inspectorOpen => _inspectorOpen;

  List<HistoryClip> get items => [
    ...history.items.where((clip) => !clip.pinned),
    ...history.items.where((clip) => clip.pinned),
  ];

  Map<LogicalKeyboardKey, HistoryClip> get shortcuts {
    final recent = history.items.where((clip) => !clip.pinned).take(9).toList();
    return {
      for (final (index, clip) in recent.indexed) _numberKeys[index]: clip,
      for (final clip in history.items.where((clip) => clip.pinned))
        ?_pinKeys[_pinnedShortcuts[clip.id]]: clip,
    };
  }

  LogicalKeyboardKey? shortcutFor(HistoryClip clip) => shortcuts.entries
      .where((entry) => entry.value.id == clip.id)
      .firstOrNull
      ?.key;

  Future<void> initialize() async {
    if (_disposed || _initialized) return;
    _initialized = true;
    _preferences = await _preferencesStore.read();
    if (_disposed) return;
    _pinnedShortcuts = await _preferencesStore.readPinnedShortcuts();
    if (_disposed) return;
    await history.initialize();
    if (_disposed) return;
    await _reconcilePinnedShortcuts();
    if (_disposed) return;
    await _refreshAccessibility();
    if (_disposed) return;
    await _host.signalReady();
    if (!_disposed) notifyListeners();
  }

  Future<void> opened(
    int presentationId, {
    bool inspectorVisible = false,
  }) async {
    if (_disposed || presentationId <= 0) return;
    _presentationId = presentationId;
    _inspectorOpen = inspectorVisible;
    _focusedClip = null;
    _presentationGeneration += 1;
    final preferences = await _preferencesStore.read();
    if (!_isCurrent(presentationId)) return;
    _preferences = preferences;
    await history.updateQuery(const HistoryQuery());
    if (!_isCurrent(presentationId)) return;
    await _reconcilePinnedShortcuts();
    if (!_isCurrent(presentationId)) return;
    await _refreshAccessibility();
    if (_inspectorOpen && _isCurrent(presentationId) && items.isNotEmpty) {
      await history.select(items.first.id);
    }
    if (_isCurrent(presentationId)) notifyListeners();
  }

  bool _isCurrent(int presentationId) =>
      !_disposed && _presentationId == presentationId;

  Future<void> search(String query) {
    final sort = query.trim().isEmpty
        ? HistorySort.newest
        : HistorySort.relevance;
    return history.updateQuery(HistoryQuery(search: query, sort: sort));
  }

  void focus(HistoryClip clip) {
    if (_focusedClip?.id == clip.id) return;
    _focusedClip = clip;
    if (_inspectorOpen) unawaited(history.select(clip.id));
    notifyListeners();
  }

  Future<void> toggleInspector() async {
    final id = _presentationId;
    if (id == null || _disposed || _activating) return;
    final visible = !_inspectorOpen;
    try {
      await _host.setInspectorVisible(presentationId: id, visible: visible);
    } on PlatformException {
      return;
    } on MissingPluginException {
      return;
    }
    if (!_isCurrent(id)) return;
    _inspectorOpen = visible;
    if (visible) {
      final clip = _focusedClip ?? items.firstOrNull;
      if (clip != null) await history.select(clip.id);
    }
    if (_isCurrent(id)) notifyListeners();
  }

  Future<void> activate(
    HistoryClip clip, {
    bool plainText = false,
    bool invertAutoPaste = false,
    bool forcePaste = false,
  }) async {
    final presentationId = _presentationId;
    if (_disposed || _activating || presentationId == null) return;
    _activating = true;
    notifyListeners();
    try {
      await history.select(clip.id);
      if (!_isCurrent(presentationId)) return;
      final copied = await history.copySelected(plainText: plainText);
      if (!copied || !_isCurrent(presentationId)) return;
      final shouldPaste = forcePaste
          ? true
          : invertAutoPaste
          ? !_preferences.autoPaste
          : _preferences.autoPaste;
      if (shouldPaste) {
        try {
          await _prepareAccessibilityForPaste(presentationId);
        } catch (_) {
          // A permission or preference failure still leaves a completed Copy.
        }
        if (!_isCurrent(presentationId)) return;
      }
      if (shouldPaste && _accessibilityGranted) {
        var pasted = false;
        try {
          pasted = await _host.paste(presentationId: presentationId);
        } on PlatformException {
          // Copy already completed; an unavailable paste channel is copy-only.
        } on MissingPluginException {
          // A disappearing native context cannot undo the completed Copy.
        }
        if (!_isCurrent(presentationId) || pasted) return;
      }
      await _closePresentation(presentationId);
    } finally {
      _activating = false;
      if (_disposed) history.dispose();
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> activateFocused({
    bool plainText = false,
    bool invertAutoPaste = false,
    bool forcePaste = false,
  }) async {
    final clip = _focusedClip ?? items.firstOrNull;
    if (clip == null) return;
    await activate(
      clip,
      plainText: plainText,
      invertAutoPaste: invertAutoPaste,
      forcePaste: forcePaste,
    );
  }

  Future<void> activateIndex(int index) async {
    final visibleItems = history.items.where((clip) => !clip.pinned).toList();
    if (index < 0 || index >= visibleItems.length) return;
    await activate(visibleItems[index]);
  }

  Future<void> toggleFocusedPin() async {
    final clip = _focusedClip;
    if (clip != null) {
      await history.togglePin(clip);
      await _reconcilePinnedShortcuts();
    }
  }

  Future<void> deleteFocused() async {
    final clip = _focusedClip;
    if (clip == null) return;
    await history.select(clip.id);
    if (await history.deleteSelected()) {
      _focusedClip = history.items.firstOrNull;
    }
  }

  Future<bool> clearUnpinned() => history.deleteAll();

  Future<void> openMainWindow() {
    _presentationId = null;
    return _disposed ? Future.value() : _host.openMainWindow();
  }

  Future<void> openSettings() {
    _presentationId = null;
    return _disposed ? Future.value() : _host.openSettings();
  }

  Future<void> close() async {
    final id = _presentationId;
    _presentationId = null;
    if (!_disposed && id != null) await _closePresentation(id);
  }

  Future<void> _closePresentation(int presentationId) async {
    if (_disposed) return;
    try {
      await _host.close(presentationId: presentationId);
    } on PlatformException {
      // Copy remains complete if the native context has already disappeared.
    } on MissingPluginException {
      // Native teardown can race with this conditional close.
    }
  }

  Future<void> quit() {
    _presentationId = null;
    return _disposed ? Future.value() : _host.quit();
  }

  Future<void> _refreshAccessibility() async {
    _accessibilityGranted = await _host.accessibilityGranted();
  }

  Future<void> _prepareAccessibilityForPaste(int presentationId) async {
    await _refreshAccessibility();
    if (!_isCurrent(presentationId) || _accessibilityGranted) return;
    final requested = await _preferencesStore.accessibilityPromptWasRequested();
    if (!_isCurrent(presentationId) || requested) return;
    // macOS returns before the user answers. Record the request before showing
    // it so a denial never prompts again on another selection or app launch.
    await _preferencesStore.markAccessibilityPromptRequested();
    if (!_isCurrent(presentationId)) return;
    _accessibilityGranted = await _host.requestAccessibility();
  }

  void _historyChanged() {
    if (_disposed) return;
    final focusedId = _focusedClip?.id;
    if (focusedId != null) {
      _focusedClip = history.items
          .where((clip) => clip.id == focusedId)
          .firstOrNull;
    }
    unawaited(_reconcilePinnedShortcuts().catchError((Object error) {}));
    notifyListeners();
  }

  Future<void> _reconcilePinnedShortcuts() {
    final operation = _shortcutSync.then((_) async {
      if (_disposed) return;
      final next = <String, String>{};
      for (final entry in _pinnedShortcuts.entries) {
        if (_pinKeys.containsKey(entry.value) &&
            !next.containsValue(entry.value)) {
          next[entry.key] = entry.value;
        }
      }
      for (final clip in history.items) {
        if (!clip.pinned) next.remove(clip.id);
      }
      for (final clip in history.items.where((clip) => clip.pinned)) {
        if (next.containsKey(clip.id)) continue;
        final available = _pinKeys.keys
            .where((key) => !next.containsValue(key))
            .toList();
        if (available.isEmpty) break;
        next[clip.id] = available[Random().nextInt(available.length)];
      }
      if (mapEquals(next, _pinnedShortcuts)) return;
      await _preferencesStore.writePinnedShortcuts(next);
      if (_disposed) return;
      _pinnedShortcuts = next;
      notifyListeners();
    });
    _shortcutSync = operation.then<void>((_) {}, onError: (Object error) {});
    return operation;
  }

  Future<void> shutdown() {
    dispose();
    return _repositoryDisposal ?? Future<void>.value();
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _presentationId = null;
    _host.setOpenedHandler(null);
    _host.setShutdownHandler(null);
    history.removeListener(_historyChanged);
    if (!_activating) history.dispose();
    final disposeRepository = _disposeRepository;
    if (disposeRepository != null) {
      _repositoryDisposal = disposeRepository();
      unawaited(_repositoryDisposal!.catchError((Object _) {}));
    }
    unawaited(_host.dispose());
    super.dispose();
  }
}

const _numberKeys = [
  LogicalKeyboardKey.digit1,
  LogicalKeyboardKey.digit2,
  LogicalKeyboardKey.digit3,
  LogicalKeyboardKey.digit4,
  LogicalKeyboardKey.digit5,
  LogicalKeyboardKey.digit6,
  LogicalKeyboardKey.digit7,
  LogicalKeyboardKey.digit8,
  LogicalKeyboardKey.digit9,
];

// Exclude select-all, quit, paste, close, undo and the pin action itself.
const _pinKeys = {
  'b': LogicalKeyboardKey.keyB,
  'c': LogicalKeyboardKey.keyC,
  'd': LogicalKeyboardKey.keyD,
  'e': LogicalKeyboardKey.keyE,
  'f': LogicalKeyboardKey.keyF,
  'g': LogicalKeyboardKey.keyG,
  'h': LogicalKeyboardKey.keyH,
  'i': LogicalKeyboardKey.keyI,
  'j': LogicalKeyboardKey.keyJ,
  'k': LogicalKeyboardKey.keyK,
  'l': LogicalKeyboardKey.keyL,
  'm': LogicalKeyboardKey.keyM,
  'n': LogicalKeyboardKey.keyN,
  'o': LogicalKeyboardKey.keyO,
  'r': LogicalKeyboardKey.keyR,
  's': LogicalKeyboardKey.keyS,
  't': LogicalKeyboardKey.keyT,
  'u': LogicalKeyboardKey.keyU,
  'x': LogicalKeyboardKey.keyX,
  'y': LogicalKeyboardKey.keyY,
};
