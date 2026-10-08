import 'package:flutter/foundation.dart' show ValueListenable;
import 'package:flutter/widgets.dart';

/// Combines the main native window with Flutter lifecycle visibility.
/// Losing focus alone must not suspend an otherwise visible application.
class ApplicationVisibility extends ValueNotifier<bool>
    with WidgetsBindingObserver {
  ApplicationVisibility({ValueListenable<bool>? windowVisible})
    : _windowVisible = windowVisible,
      super(true) {
    WidgetsBinding.instance.addObserver(this);
    _windowVisible?.addListener(_update);
    _lifecycleState = WidgetsBinding.instance.lifecycleState;
    _update();
  }

  final ValueListenable<bool>? _windowVisible;
  AppLifecycleState? _lifecycleState;

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    _lifecycleState = state;
    _update();
  }

  void _update() {
    value =
        (_windowVisible?.value ?? true) &&
        _lifecycleState != AppLifecycleState.hidden &&
        _lifecycleState != AppLifecycleState.paused;
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _windowVisible?.removeListener(_update);
    super.dispose();
  }
}
