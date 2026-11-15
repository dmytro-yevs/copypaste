import 'dart:async';

import 'package:flutter/foundation.dart';

/// Suspends presentation after continuous hidden time without owning runtime
/// services. Visible windows, including unfocused ones, cancel the deadline.
///
/// A launch that starts hidden has never been seen, so it uses
/// [startHiddenDelay] until the first suspension or until the user shows the
/// window, instead of keeping the whole presentation for [hiddenDelay].
class UiMemoryController extends ChangeNotifier {
  UiMemoryController({
    required ValueListenable<bool> visible,
    required bool Function() canSuspend,
    bool startHidden = false,
  }) : _visible = visible,
       _canSuspend = canSuspend,
       _delay = startHidden ? startHiddenDelay : hiddenDelay {
    _visible.addListener(_visibilityChanged);
    _visibilityChanged();
  }

  static const hiddenDelay = Duration(seconds: 60);
  static const startHiddenDelay = Duration(seconds: 2);

  final ValueListenable<bool> _visible;
  final bool Function() _canSuspend;
  Duration _delay;
  bool _seenHidden = false;
  Timer? _deadline;
  bool _suspended = false;

  bool get suspended => _suspended;

  void _visibilityChanged() {
    _deadline?.cancel();
    if (_visible.value) {
      // The window reports visible before a hidden launch hides it.
      if (_seenHidden) _delay = hiddenDelay;
      if (_suspended) {
        _suspended = false;
        notifyListeners();
      }
    } else if (!_suspended) {
      _seenHidden = true;
      _schedule();
    }
  }

  void _schedule() {
    _deadline = Timer(_delay, () {
      if (_visible.value) return;
      // Preserve in-progress dialogs, pairing, and user-initiated operations.
      if (!_canSuspend()) {
        _schedule();
        return;
      }
      _suspended = true;
      _delay = hiddenDelay;
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
