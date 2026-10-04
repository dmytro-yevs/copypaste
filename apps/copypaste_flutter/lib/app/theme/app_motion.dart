import 'package:flutter/widgets.dart';
import 'package:flutter_animate/flutter_animate.dart';

/// Canonical timing, easing, and reusable effects for application-owned motion.
///
/// Stock shadcn components keep their package-owned transitions. New CopyPaste
/// motion must use this contract instead of declaring local durations, curves,
/// controllers, or effect chains.
abstract final class AppMotion {
  static const Duration instant = Duration.zero;
  static const Duration quick = Duration(milliseconds: 100);
  static const Duration standard = Duration(milliseconds: 200);
  static const Duration emphasized = Duration(milliseconds: 300);
  static const Duration settingsHighlightHold = Duration(milliseconds: 1200);

  static const Curve standardCurve = Curves.easeInOutCubic;
  static const Curve enterCurve = Curves.easeOutCubic;
  static const Curve exitCurve = Curves.easeInCubic;

  static const List<Effect<dynamic>> fadeIn = <Effect<dynamic>>[
    FadeEffect(duration: standard, curve: enterCurve),
  ];

  static const List<Effect<dynamic>> fadeUp = <Effect<dynamic>>[
    FadeEffect(duration: standard, curve: enterCurve),
    SlideEffect(
      begin: Offset(0, 0.02),
      end: Offset.zero,
      duration: standard,
      curve: enterCurve,
    ),
  ];

  static const List<Effect<dynamic>> scaleIn = <Effect<dynamic>>[
    FadeEffect(duration: standard, curve: enterCurve),
    ScaleEffect(
      begin: Offset(0.98, 0.98),
      end: Offset(1, 1),
      duration: standard,
      curve: enterCurve,
    ),
  ];

  static const List<Effect<dynamic>> fadeOut = <Effect<dynamic>>[
    FadeEffect(begin: 1, end: 0, duration: quick, curve: exitCurve),
  ];

  static bool reducedMotionOf(BuildContext context) =>
      MediaQuery.disableAnimationsOf(context);

  static Duration resolve(BuildContext context, Duration duration) =>
      reducedMotionOf(context) ? instant : duration;

  static Duration resolveDisabled(bool disabled, Duration duration) =>
      disabled ? instant : duration;

  static List<Effect<dynamic>> resolveEffects(
    BuildContext context,
    List<Effect<dynamic>> effects,
  ) => reducedMotionOf(context) ? const <Effect<dynamic>>[] : effects;

  /// Aligns flutter_animate's implicit defaults with the CopyPaste contract.
  static void configureLibrary() {
    Animate.defaultDuration = standard;
    Animate.defaultCurve = standardCurve;
  }
}
