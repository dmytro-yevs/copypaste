import 'dart:async';

import 'package:flutter/foundation.dart';

/// Suspends presentation after continuous hidden time without owning runtime
/// services. Visible windows, including unfocused ones, cancel the deadline.
class UiMemoryController extends ChangeNotifier {
  UiMemoryController({
    required ValueListenable<bool> visible,
    required bool Function() canSuspend,
  }) : _visible = visible,
       _canSuspend = canSuspend {
    _visible.addListener(_visibilityChanged);
    _visibilityChanged();
  }

  static const hiddenDelay = Duration(seconds: 60);

  final ValueListenable<bool> _visible;
  final bool Function() _canSuspend;
  Timer? _deadline;
  bool _suspended = false;

  bool get suspended => _suspended;

  void _visibilityChanged() {
    _deadline?.cancel();
    if (_visible.value) {
      if (_suspended) {
        _suspended = false;
        notifyListeners();
      }
    } else if (!_suspended) {
      _schedule();
    }
  }

  void _schedule() {
    _deadline = Timer(hiddenDelay, () {
      if (_visible.value) return;
      // Preserve in-progress dialogs, pairing, and user-initiated operations.
      if (!_canSuspend()) {
        _schedule();
        return;
      }
      _suspended = true;
      notifyListeners();
    });
  }

  @override
  void dispose() {
    _deadline?.cancel();
    _visible.removeListener(_visibilityChanged);
    super.dispose();
  }
}
