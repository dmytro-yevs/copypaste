import 'dart:async';

import 'package:flutter/foundation.dart';

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

  bool get autoPaste => _preferences.autoPaste;
  bool get accessibilityGranted => _accessibilityGranted;
  bool get activating => _activating;
  int get presentationGeneration => _presentationGeneration;
  HistoryClip? get focusedClip => _focusedClip;

  Future<void> initialize() async {
    if (_initialized) return;
    _initialized = true;
    _preferences = await _preferencesStore.read();
    await history.initialize();
    await _refreshAccessibility(prompt: false);
    notifyListeners();
  }

  Future<void> opened() async {
    _preferences = await _preferencesStore.read();
    _focusedClip = null;
    _presentationGeneration += 1;
    await history.updateQuery(const HistoryQuery());
    await _refreshAccessibility(prompt: _preferences.autoPaste);
    notifyListeners();
  }

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
    if (_activating) return;
    _activating = true;
    notifyListeners();
    try {
      await history.select(clip.id);
      final copied = await history.copySelected(plainText: plainText);
      if (!copied) return;
      final shouldPaste = forcePaste
          ? true
          : invertAutoPaste
          ? !_preferences.autoPaste
          : _preferences.autoPaste;
      if (shouldPaste && _accessibilityGranted && await _host.paste()) {
        return;
      }
      await _host.close();
    } finally {
      _activating = false;
      notifyListeners();
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

  Future<void> openMainWindow() => _host.openMainWindow();

  Future<void> openSettings() => _host.openSettings();

  Future<void> close() => _host.close();

  Future<void> quit() => _host.quit();

  Future<void> _refreshAccessibility({required bool prompt}) async {
    _accessibilityGranted = await _host.accessibilityGranted();
    if (prompt && !_accessibilityGranted) {
      _accessibilityGranted = await _host.requestAccessibility();
    }
  }

  void _historyChanged() {
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
    history.removeListener(_historyChanged);
    history.dispose();
    final disposeRepository = _disposeRepository;
    if (disposeRepository != null) unawaited(disposeRepository());
    unawaited(_host.dispose());
    super.dispose();
  }
}
