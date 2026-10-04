import 'dart:io';

import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_animate/flutter_animate.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('configures flutter_animate with the shared defaults', () {
    AppMotion.configureLibrary();

    expect(Animate.defaultDuration, AppMotion.standard);
    expect(Animate.defaultCurve, AppMotion.standardCurve);
  });

  test('exposes a restrained reusable effect set', () {
    expect(AppMotion.fadeIn, hasLength(1));
    expect(AppMotion.fadeIn.single, isA<FadeEffect>());
    expect(AppMotion.fadeUp, contains(isA<SlideEffect>()));
    expect(AppMotion.scaleIn, contains(isA<ScaleEffect>()));
    expect(AppMotion.fadeOut.single, isA<FadeEffect>());
  });

  testWidgets('removes application-owned motion when animations are disabled', (
    tester,
  ) async {
    late Duration resolvedDuration;
    late List<Effect<dynamic>> resolvedEffects;

    await tester.pumpWidget(
      MediaQuery(
        data: const MediaQueryData(disableAnimations: true),
        child: Builder(
          builder: (context) {
            resolvedDuration = AppMotion.resolve(context, AppMotion.emphasized);
            resolvedEffects = AppMotion.resolveEffects(
              context,
              AppMotion.fadeUp,
            );
            return const SizedBox.shrink();
          },
        ),
      ),
    );

    expect(resolvedDuration, Duration.zero);
    expect(resolvedEffects, isEmpty);
  });

  testWidgets('keeps application-owned motion when animations are enabled', (
    tester,
  ) async {
    late Duration resolvedDuration;
    late List<Effect<dynamic>> resolvedEffects;

    await tester.pumpWidget(
      MediaQuery(
        data: const MediaQueryData(),
        child: Builder(
          builder: (context) {
            resolvedDuration = AppMotion.resolve(context, AppMotion.standard);
            resolvedEffects = AppMotion.resolveEffects(
              context,
              AppMotion.fadeIn,
            );
            return const SizedBox.shrink();
          },
        ),
      ),
    );

    expect(resolvedDuration, AppMotion.standard);
    expect(resolvedEffects, same(AppMotion.fadeIn));
  });

  test('production motion uses AppMotion instead of local implementations', () {
    final localTiming = RegExp(
      r'\b(?:duration|transitionDuration|reverseTransitionDuration|showDuration|dismissDuration)\s*:\s*(?:const\s+)?Duration\(',
    );
    final localCurve = RegExp(r'\bcurve\s*:\s*Curves\.');
    final customImplementation = RegExp(
      r'\b(?:AnimationController|Tween(?:Sequence)?|AnimatedContainer|AnimatedOpacity|AnimatedPositioned|AnimatedAlign|AnimatedPadding|AnimatedSize|AnimatedSwitcher|AnimatedCrossFade|AnimatedSlide|AnimatedScale|AnimatedRotation|FadeTransition|SlideTransition|ScaleTransition)\b',
    );
    final offenders = <String>[];

    for (final entity in Directory('lib').listSync(recursive: true)) {
      if (entity is! File || !entity.path.endsWith('.dart')) continue;
      final path = entity.path.replaceAll('\\', '/');
      if (path.endsWith('app/theme/app_motion.dart') ||
          path.contains('/generated/')) {
        continue;
      }
      final source = entity.readAsStringSync();
      if (localTiming.hasMatch(source) ||
          localCurve.hasMatch(source) ||
          customImplementation.hasMatch(source)) {
        offenders.add(entity.path);
      }
    }

    expect(
      offenders,
      isEmpty,
      reason:
          'Application-owned motion must use AppMotion tokens and reusable effects.',
    );
  });
}
