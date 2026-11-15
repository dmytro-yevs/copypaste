import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

import '../../platform/desktop/window_material.dart';

/// Tells the theme whether the window material shows through the window
/// surfaces. Without it, every surface stays opaque.
class AppWindowMaterial
    extends InheritedNotifier<ValueListenable<WindowMaterialState>> {
  const AppWindowMaterial({
    super.key,
    required ValueListenable<WindowMaterialState> state,
    required super.child,
  }) : super(notifier: state);

  static bool translucentOf(BuildContext context) =>
      context
          .dependOnInheritedWidgetOfExactType<AppWindowMaterial>()
          ?.notifier
          ?.value
          .translucent ??
      false;
}
