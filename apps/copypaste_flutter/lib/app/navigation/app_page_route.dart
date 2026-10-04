import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../theme/app_motion.dart';

/// A shadcn route that removes transitions when motion is disabled.
class AppPageRoute<T> extends ShadcnPageRoute<T> {
  AppPageRoute({
    required super.builder,
    required this.disableAnimations,
    super.settings,
  }) : super(
         transitionDuration: AppMotion.resolveDisabled(
           disableAnimations,
           AppMotion.emphasized,
         ),
       );

  final bool disableAnimations;
}
