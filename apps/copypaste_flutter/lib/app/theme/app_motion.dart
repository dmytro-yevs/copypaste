import 'dart:math' as math;
import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter/physics.dart';
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
  static const Duration navigation = Duration(milliseconds: 320);
  static const Duration navigationHold = Duration(milliseconds: 375);
  static const Duration navigationSettle = Duration(milliseconds: 450);
  static const double navigationPressScale = 1.012;
  static const double navigationHoldScale = 1.019;
  static const double navigationPressVelocity = -0.45;
  static const double navigationReleaseVelocity = 0.25;
  static const double navigationMinimumSelectionScale = 0.6;
  static final navigationPressSpring = SpringDescription.withDampingRatio(
    mass: 1,
    stiffness: 250,
    ratio: 0.25,
  );
  static final navigationSelectorSpring = SpringDescription.withDampingRatio(
    mass: 1,
    stiffness: 1500,
    ratio: 0.5,
  );
  static const double navigationIconPeakScale = 1.08;
  static double navigationIconScale(double progress) =>
      1 + (navigationIconPeakScale - 1) * math.sin(math.pi * progress);
  static const Curve navigationCurve = Curves.decelerate;

  static Widget navigationSelection(
    BuildContext context, {
    required double weight,
    required bool directlyManipulated,
    required ValueWidgetBuilder<double> builder,
  }) => TweenAnimationBuilder<double>(
    tween: Tween(begin: weight, end: weight),
    duration: directlyManipulated ? instant : resolve(context, navigation),
    curve: navigationCurve,
    builder: builder,
  );

  static Widget navigationIconResponse(
    BuildContext context, {
    required bool selected,
    required Widget child,
  }) {
    if (reducedMotionOf(context)) return child;
    final weight = selected ? 1.0 : 0.0;
    return TweenAnimationBuilder<double>(
      tween: Tween(begin: weight, end: weight),
      duration: navigation,
      curve: navigationCurve,
      builder: (context, progress, child) =>
          Transform.scale(scale: navigationIconScale(progress), child: child),
      child: child,
    );
  }

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

/// Owns the canonical spring animations used by the compact navigation surface.
class AppNavigationMotion {
  AppNavigationMotion({required TickerProvider vsync})
    : press = AnimationController.unbounded(vsync: vsync, value: 1),
      selector = AnimationController.unbounded(vsync: vsync);

  final AnimationController press;
  final AnimationController selector;

  Widget pressResponse({required Widget child}) =>
      ScaleTransition(scale: press, child: child);

  void pressTo(
    double target, {
    required double velocity,
    required bool reduced,
  }) {
    if (reduced) {
      press.value = 1;
      return;
    }
    unawaited(
      press.animateWith(
        SpringSimulation(
          AppMotion.navigationPressSpring,
          press.value,
          target,
          velocity,
        ),
      ),
    );
  }

  void settleSelector(double target) {
    unawaited(
      selector.animateWith(
        SpringSimulation(
          AppMotion.navigationSelectorSpring,
          selector.value,
          target,
          0,
        ),
      ),
    );
  }

  void dispose() {
    press.dispose();
    selector.dispose();
  }
}
