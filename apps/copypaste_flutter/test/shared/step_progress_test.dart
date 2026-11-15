import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/shared/step_progress.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  const steps = ['Invite', 'Connect', 'Confirm', 'Paired'];

  Future<void> pump(
    WidgetTester tester, {
    required int current,
    String? failure,
    double width = 640,
  }) => tester.pumpWidget(
    ShadcnApp(
      theme: AppTheme.light,
      builder: AppTheme.builder,
      home: Scaffold(
        child: Center(
          child: SizedBox(
            width: width,
            child: StepProgress(
              steps: steps,
              current: current,
              failure: failure,
            ),
          ),
        ),
      ),
    ),
  );

  testWidgets('names the current step, its position and checks done steps', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    await pump(tester, current: 1);
    expect(find.text('Connect'), findsOneWidget);
    expect(find.text('Step 2 of 4'), findsOneWidget);
    expect(find.text('Confirm'), findsNothing);
    expect(find.byIcon(LucideIcons.check), findsOneWidget);
    expect(
      tester.getSemantics(find.byType(StepProgress)),
      isSemantics(label: 'Step 2 of 4: Connect', isLiveRegion: true),
    );
    final height = tester.getSize(find.byType(StepProgress)).height;

    // A failed step shows a cross and keeps the same height.
    await pump(tester, current: 2, failure: 'Not paired');
    await tester.pumpAndSettle();
    expect(find.byIcon(LucideIcons.check), findsNWidgets(2));
    expect(find.byIcon(LucideIcons.x), findsOneWidget);
    expect(
      tester.getSemantics(find.byType(StepProgress)),
      isSemantics(label: 'Step 3 of 4: Confirm', value: 'Not paired'),
    );
    expect(tester.getSize(find.byType(StepProgress)).height, height);

    // Every step done names the last one.
    await pump(tester, current: steps.length);
    await tester.pumpAndSettle();
    expect(find.byIcon(LucideIcons.check), findsNWidgets(4));
    expect(find.text('Step 4 of 4'), findsOneWidget);
    expect(tester.getSize(find.byType(StepProgress)).height, height);
    semantics.dispose();
  });

  testWidgets('fits 240 pixels with doubled text', (tester) async {
    tester.platformDispatcher.textScaleFactorTestValue = 2.0;
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    await pump(tester, current: 1, width: 240);
    expect(tester.takeException(), isNull);
  });
}
