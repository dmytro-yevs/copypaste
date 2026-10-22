import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/features/transfers/file_operations_button_progress.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets(
    'perimeter progress handles determinate, unknown, idle, and reduced motion states',
    (tester) async {
      await tester.pumpWidget(_host(active: true, progress: 0.25));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(tester.hasRunningAnimations, isFalse);

      await tester.pumpWidget(_host(active: true, progress: 0.75));
      await tester.pump();
      expect(tester.hasRunningAnimations, isTrue);
      await tester.pump(AppMotion.quick);
      expect(tester.hasRunningAnimations, isFalse);

      await tester.pumpWidget(_host(active: true));
      await tester.pump();
      expect(tester.takeException(), isNull);
      expect(tester.hasRunningAnimations, isFalse);

      await tester.pumpWidget(
        _host(active: true, progress: 0.75, disableAnimations: true),
      );
      await tester.pump();
      expect(tester.hasRunningAnimations, isFalse);

      await tester.pumpWidget(_host());
      await tester.pump();
      expect(find.byType(FileOperationsButtonProgress), findsNothing);
      expect(tester.takeException(), isNull);

      await tester.pumpWidget(
        _host(active: true, progress: 0.5, disableAnimations: true),
      );
      await tester.pump();
      expect(tester.takeException(), isNull);
      expect(tester.hasRunningAnimations, isFalse);
    },
  );
}

Widget _host({
  bool active = false,
  double? progress,
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
            : FileOperationsButtonProgress(
                progress: progress,
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
);
