import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/features/transfers/file_operations_button_progress.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets(
    'perimeter progress handles determinate, unknown, idle, and reduced motion states',
    (tester) async {
      final progress = ValueNotifier<double?>(0.25);
      addTearDown(progress.dispose);
      await tester.pumpWidget(_host(active: true, progress: progress));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      progress.value = 0.75;
      await tester.pump();
      final transition = find.descendant(
        of: find.byType(FileOperationsButtonProgress),
        matching: find.byType(TweenAnimationBuilder<double>),
      );
      expect(transition, findsOneWidget);
      final transitionWidget = tester.widget<TweenAnimationBuilder<double>>(
        transition,
      );
      expect(transitionWidget.duration, AppMotion.quick);
      expect(transitionWidget.curve, AppMotion.standardCurve);
      await tester.pumpAndSettle();

      progress.value = null;
      await tester.pump();
      expect(tester.takeException(), isNull);

      final reducedProgress = ValueNotifier<double?>(0.5);
      addTearDown(reducedProgress.dispose);
      await tester.pumpWidget(
        _host(active: true, progress: reducedProgress, disableAnimations: true),
      );
      await tester.pumpAndSettle();
      reducedProgress.value = 0.75;
      await tester.pump();
      final reducedTransition = tester.widget<TweenAnimationBuilder<double>>(
        find.descendant(
          of: find.byType(FileOperationsButtonProgress),
          matching: find.byType(TweenAnimationBuilder<double>),
        ),
      );
      expect(reducedTransition.duration, AppMotion.instant);
      await tester.pumpAndSettle();

      final idleProgress = ValueNotifier<double?>(null);
      addTearDown(idleProgress.dispose);
      await tester.pumpWidget(_host(progress: idleProgress, active: false));
      await tester.pump();
      expect(find.byType(FileOperationsButtonProgress), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );
}

Widget _host({
  required ValueListenable<double?> progress,
  bool active = false,
  bool disableAnimations = false,
}) => ShadcnApp(
  theme: AppTheme.dark,
  builder: (context, child) => AppTheme.builder(context, child),
  home: MediaQuery(
    data: MediaQueryData(disableAnimations: disableAnimations),
    child: Scaffold(
      child: Center(
        child: !active
            ? Button.secondary(
                onPressed: () {},
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.secondaryIcon(),
                ),
                child: const Icon(LucideIcons.download),
              )
            : ValueListenableBuilder<double?>(
                valueListenable: progress,
                builder: (context, value, _) => FileOperationsButtonProgress(
                  progress: value,
                  child: Button.secondary(
                    onPressed: () {},
                    style: AppTheme.controlButtonStyle(
                      const ButtonStyle.secondaryIcon(),
                    ),
                    child: const Icon(LucideIcons.download),
                  ),
                ),
              ),
      ),
    ),
  ),
);
