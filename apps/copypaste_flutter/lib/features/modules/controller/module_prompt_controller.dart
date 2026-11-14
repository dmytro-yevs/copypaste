import 'dart:async';

import 'package:flutter/foundation.dart';

import '../models/module_prompt.dart';
import '../repository/modules_repository.dart';

/// Module prompts waiting for an answer. It rereads them on each change
/// event and drops a prompt locally once it expires. With a [notifier], a
/// prompt that arrives while the app is hidden posts a system notification,
/// and activating it shows the app, where the prompt presenter takes over.
class ModulePromptController extends ChangeNotifier {
  ModulePromptController({
    required ModulePromptsRepository repository,
    required String? Function(String moduleId) moduleTitle,
    ModulePromptNotifier? notifier,
    ValueListenable<bool>? visible,
    VoidCallback? showApp,
    DateTime Function() now = DateTime.now,
  }) : _repository = repository,
       _moduleTitle = moduleTitle,
       _notifier = notifier,
       _visible = visible,
       _showApp = showApp,
       _now = now {
    _changes = repository.changes().listen(
      (_) => unawaited(_reload()),
      onError: (Object _) {},
    );
    _opened = notifier?.opened.listen((_) => _openFromNotification());
    visible?.addListener(_syncNotification);
    scheduleMicrotask(() {
      if (!_disposed) unawaited(_reload());
    });
  }

  final ModulePromptsRepository _repository;
  final String? Function(String moduleId) _moduleTitle;
  final ModulePromptNotifier? _notifier;
  final ValueListenable<bool>? _visible;
  final VoidCallback? _showApp;
  final DateTime Function() _now;
  late final StreamSubscription<void> _changes;
  late final StreamSubscription<String>? _opened;
  String? _notified;
  List<ModulePrompt> _pending = const [];
  final _resolving = <String>{};
  Timer? _expiry;
  int _generation = 0;
  bool _disposed = false;

  List<ModulePrompt> get pending => _pending;

  /// The prompt to show, oldest first.
  ModulePrompt? get current =>
      _pending.where((prompt) => !_resolving.contains(prompt.id)).firstOrNull;

  /// The asking module's name, or null when it is no longer installed.
  String? moduleTitle(ModulePrompt prompt) => _moduleTitle(prompt.moduleId);

  Future<void> _reload() async {
    final generation = ++_generation;
    List<ModulePrompt> pending;
    try {
      pending = await _repository.pending();
    } on Object {
      pending = const [];
    }
    if (_disposed || generation != _generation) return;
    _pending = pending;
    _resolving.removeWhere((id) => !pending.any((prompt) => prompt.id == id));
    _expire();
  }

  void _expire() {
    _expiry?.cancel();
    final now = _now();
    _pending = [
      for (final prompt in _pending)
        if (prompt.expiresAt.isAfter(now)) prompt,
    ];
    final next = _pending
        .map((prompt) => prompt.expiresAt)
        .fold<DateTime?>(
          null,
          (soonest, at) =>
              soonest == null || at.isBefore(soonest) ? at : soonest,
        );
    if (next != null) _expiry = Timer(next.difference(now), _expire);
    _syncNotification();
    notifyListeners();
  }

  void _syncNotification() {
    final notifier = _notifier;
    if (notifier == null || _disposed) return;
    final prompt = current;
    if (prompt == null || (_visible?.value ?? true)) {
      _dismissNotification();
      return;
    }
    if (_notified == prompt.id) return;
    _notified = prompt.id;
    unawaited(notifier.show(prompt).catchError((Object _) {}));
  }

  void _dismissNotification() {
    if (_notified == null) return;
    _notified = null;
    unawaited(_notifier?.dismiss().catchError((Object _) {}));
  }

  void _openFromNotification() {
    if (_disposed) return;
    _dismissNotification();
    if (!(_visible?.value ?? true)) _showApp?.call();
  }

  /// Sends the answer; a null [choice] dismisses. A failure leaves the
  /// prompt pending, so it returns until answered or expired.
  Future<bool> resolve(ModulePrompt prompt, String? choice) async {
    if (!_resolving.add(prompt.id)) return false;
    notifyListeners();
    try {
      await _repository.answer(prompt.id, choice);
      _pending = [
        for (final entry in _pending)
          if (entry.id != prompt.id) entry,
      ];
      return true;
    } on Object {
      return false;
    } finally {
      _resolving.remove(prompt.id);
      if (!_disposed) {
        _syncNotification();
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _expiry?.cancel();
    _visible?.removeListener(_syncNotification);
    unawaited(_changes.cancel());
    unawaited(_opened?.cancel());
    _dismissNotification();
    _disposed = true;
    super.dispose();
  }
}
