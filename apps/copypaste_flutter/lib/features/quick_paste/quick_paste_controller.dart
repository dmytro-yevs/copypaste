import 'dart:async';

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

  bool get autoPaste => _preferences.autoPaste;
  bool get accessibilityGranted => _accessibilityGranted;
  bool get activating => _activating;
  int get presentationGeneration => _presentationGeneration;
  HistoryClip? get focusedClip => _focusedClip;

  Future<void> initialize() async {
    if (_disposed || _initialized) return;
    _initialized = true;
    _preferences = await _preferencesStore.read();
    if (_disposed) return;
    await history.initialize();
    if (_disposed) return;
    await _refreshAccessibility(prompt: false);
    if (!_disposed) notifyListeners();
  }

  Future<void> opened(int presentationId) async {
    if (_disposed || presentationId <= 0) return;
    _presentationId = presentationId;
    _focusedClip = null;
    _presentationGeneration += 1;
    final preferences = await _preferencesStore.read();
    if (!_isCurrent(presentationId)) return;
    _preferences = preferences;
    await history.updateQuery(const HistoryQuery());
    if (!_isCurrent(presentationId)) return;
    await _refreshAccessibility(prompt: _preferences.autoPaste);
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
    notifyListeners();
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
    final clip = _focusedClip ?? history.items.firstOrNull;
    if (clip == null) return;
    await activate(
      clip,
      plainText: plainText,
      invertAutoPaste: invertAutoPaste,
      forcePaste: forcePaste,
    );
  }

  Future<void> activateIndex(int index) async {
    if (index < 0 || index >= history.items.length) return;
    await activate(history.items[index]);
  }

  Future<void> toggleFocusedPin() async {
    final clip = _focusedClip;
    if (clip != null) await history.togglePin(clip);
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

  Future<void> _refreshAccessibility({required bool prompt}) async {
    _accessibilityGranted = await _host.accessibilityGranted();
    if (!_disposed && prompt && !_accessibilityGranted) {
      _accessibilityGranted = await _host.requestAccessibility();
    }
  }

  void _historyChanged() {
    if (_disposed) return;
    final focusedId = _focusedClip?.id;
    if (focusedId != null) {
      _focusedClip = history.items
          .where((clip) => clip.id == focusedId)
          .firstOrNull;
    }
    notifyListeners();
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _presentationId = null;
    _host.setOpenedHandler(null);
    history.removeListener(_historyChanged);
    if (!_activating) history.dispose();
    final disposeRepository = _disposeRepository;
    if (disposeRepository != null) unawaited(disposeRepository());
    unawaited(_host.dispose());
    super.dispose();
  }
}
