import 'dart:async';

import 'package:flutter/foundation.dart';

import '../models/ai_tools_models.dart';
import '../repository/ai_approval_notifier.dart';
import '../repository/ai_tools_repository.dart';

/// AI tool requests waiting at this desktop. It re-reads them on each
/// approval event, posts a system notification for a request that arrives
/// while the window is hidden, and shows the window when that notification
/// is activated; the approval presenter then shows the dialog on top.
/// Desktop only: Android never serves AI tools.
class AiApprovalController extends ChangeNotifier {
  AiApprovalController({
    required AiToolsRepository repository,
    required AiApprovalNotifier notifier,
    required ValueListenable<bool> visible,
    required VoidCallback showWindow,
  }) : _repository = repository,
       _notifier = notifier,
       _visible = visible,
       _showWindow = showWindow {
    _changes = repository.approvalChanges().listen(
      (_) => unawaited(_reload()),
      onError: (Object _) {},
    );
    _opened = notifier.opened.listen((_) => _openFromNotification());
    visible.addListener(_visibilityChanged);
    scheduleMicrotask(() {
      if (!_disposed) unawaited(_reload());
    });
  }

  final AiToolsRepository _repository;
  final AiApprovalNotifier _notifier;
  final ValueListenable<bool> _visible;
  final VoidCallback _showWindow;
  late final StreamSubscription<void> _changes;
  late final StreamSubscription<String> _opened;
  List<AiApprovalRequest> _pending = const [];
  AiToolLookback _defaultLookback = AiToolLookback.week;
  String? _notified;
  final _resolving = <String>{};
  int _generation = 0;
  bool _disposed = false;

  List<AiApprovalRequest> get pending => _pending;

  /// The request a prompt is shown for, oldest first.
  AiApprovalRequest? get current => _pending
      .where((request) => !_resolving.contains(request.requestId))
      .firstOrNull;

  /// The policy's lookback for new connections, the prompt's default.
  AiToolLookback get defaultLookback => _defaultLookback;

  Future<void> _reload() async {
    final generation = ++_generation;
    try {
      final pending = await _repository.pendingApprovals();
      var lookback = _defaultLookback;
      if (pending.any((request) => request.kind == AiApprovalKind.access)) {
        lookback = (await _repository.policy()).defaultLookback;
      }
      if (_disposed || generation != _generation) return;
      _pending = pending;
      _defaultLookback = lookback;
      _resolving.removeWhere(
        (id) => !pending.any((request) => request.requestId == id),
      );
    } on Object {
      if (_disposed || generation != _generation) return;
      _pending = const [];
    }
    _syncNotification();
    notifyListeners();
  }

  void _syncNotification() {
    final request = current;
    if (request == null || _visible.value) {
      _dismissNotification();
      return;
    }
    if (_notified == request.requestId) return;
    _notified = request.requestId;
    unawaited(_notifier.show(request).catchError((Object _) {}));
  }

  void _dismissNotification() {
    if (_notified == null) return;
    _notified = null;
    unawaited(_notifier.dismiss().catchError((Object _) {}));
  }

  void _visibilityChanged() {
    if (_visible.value) {
      _dismissNotification();
    } else {
      _syncNotification();
    }
  }

  void _openFromNotification() {
    if (_disposed) return;
    _dismissNotification();
    if (!_visible.value) _showWindow();
  }

  /// Sends the decision; a failure leaves the request pending, so its
  /// prompt returns until it is decided or times out.
  Future<bool> resolve(
    AiApprovalRequest request,
    AiApprovalDecision decision,
  ) async {
    if (!_resolving.add(request.requestId)) return false;
    notifyListeners();
    try {
      switch (request.kind) {
        case AiApprovalKind.access when decision.allowed:
          await _repository.approve(
            request.requestId,
            scopes: decision.scopes,
            lookback: decision.lookback,
          );
        case AiApprovalKind.access:
          await _repository.deny(request.requestId);
        case AiApprovalKind.volume:
          await _repository.decideVolume(
            request.requestId,
            allow: decision.allowed,
          );
      }
      _pending = [
        for (final entry in _pending)
          if (entry.requestId != request.requestId) entry,
      ];
      return true;
    } on Object {
      return false;
    } finally {
      _resolving.remove(request.requestId);
      if (!_disposed) {
        _syncNotification();
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _visible.removeListener(_visibilityChanged);
    unawaited(_changes.cancel());
    unawaited(_opened.cancel());
    _dismissNotification();
    super.dispose();
  }
}
