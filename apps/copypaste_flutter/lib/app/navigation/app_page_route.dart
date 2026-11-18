import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../theme/app_motion.dart';

/// A shadcn route that removes transitions when motion is disabled.
class AppPageRoute<T> extends ShadcnPageRoute<T> {
  AppPageRoute({
    required super.builder,
    required this.disableAnimations,
    super.settings,
    bool? requestFocus,
    this.passive,
  }) : _requestFocus = requestFocus,
       super(
         transitionDuration: AppMotion.resolveDisabled(
           disableAnimations,
           AppMotion.emphasized,
         ),
       );

  final bool disableAnimations;
  final bool? _requestFocus;

  /// Whether the page currently holds no unsaved input, so it may close while
  /// the app is hidden. Null means it never may.
  final ValueGetter<bool>? passive;

  /// Null follows the navigator.
  @override
  bool get requestFocus => _requestFocus ?? super.requestFocus;
}
